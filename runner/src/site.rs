//! `runner site`: the static leaderboard, from a results folder.
//!
//! One self-contained `index.html`, in the "podium" design. The results go into the page
//! as JSON, and a small script draws it: per category, a podium, the progress chart
//! (program size over time, with a record line), the table with (i) explanations, and
//! the entries that don't pass.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde_json::{json, Value};

use crate::entries::{Meta, CATEGORIES};
use crate::results::EntryResult;
use crate::spend::{load_ledger, Ledger};

/// How each category is described on the page.
fn scheme(category: &str) -> &'static str {
    match category {
        "shrincs-l-stateful" => "SHRINCS-L, stateful path · n = 16, W = 4, 207 signatures per key",
        _ => "",
    }
}

/// An entry's signet transaction: from `runner spend`'s ledger when it has spent this same
/// verifier, otherwise from the entry's own `entry.json`. Read at build time rather than
/// from the results, so a link appears without testing the entry again. `None` unless
/// it's a 64-digit hex txid.
fn signet_tx(root: &Path, ledger: &Ledger, e: &EntryResult) -> Option<String> {
    let key = format!("{}/{}", e.category, e.slug);
    if let Some(txid) = ledger
        .get(&key)
        .filter(|r| r.sha256 == e.sha256)
        .and_then(|r| r.spend_txid.clone())
    {
        return Some(txid);
    }
    let path = &e.path;
    let meta: Meta =
        serde_json::from_str(&std::fs::read_to_string(root.join(path).join("entry.json")).ok()?)
            .ok()?;
    let txid = meta.signet_tx?.trim().to_lowercase();
    (txid.len() == 64 && txid.chars().all(|c| c.is_ascii_hexdigit())).then_some(txid)
}

pub fn build(
    results: &Path,
    out: &Path,
    source_base: &str,
    root: &Path,
    explorer: &str,
) -> Result<()> {
    let ledger = load_ledger(root)?;
    let mut by_category: BTreeMap<String, Vec<EntryResult>> = BTreeMap::new();
    for (category, _) in CATEGORIES {
        let Ok(files) = std::fs::read_dir(results.join(category)) else {
            continue;
        };
        for file in files.filter_map(|f| f.ok().map(|f| f.path())) {
            if file.extension().is_some_and(|x| x == "json") {
                let r: EntryResult = serde_json::from_str(&std::fs::read_to_string(&file)?)
                    .with_context(|| format!("{}", file.display()))?;
                by_category.entry(category.to_string()).or_default().push(r);
            }
        }
    }

    let mut tested_at = 0;
    let mut qs = Vec::new();
    let categories: Vec<Value> = by_category
        .into_iter()
        .map(|(category, mut entries)| {
            entries.sort_by(|a, b| a.score_wu.cmp(&b.score_wu).then(a.slug.cmp(&b.slug)));
            for e in &entries {
                tested_at = tested_at.max(e.run.tested_at);
                qs.clone_from(&e.run.qs);
            }
            let date = |e: &EntryResult| {
                e.submitted_at
                    .clone()
                    .unwrap_or_else(|| day(e.run.tested_at))
            };
            let source = |e: &EntryResult| {
                format!(
                    "{}/{}/verifier.simf",
                    source_base.trim_end_matches('/'),
                    e.path
                )
            };
            let passing: Vec<Value> = entries
                .iter()
                .filter(|e| e.passed)
                .map(|e| {
                    json!({
                        "slug": e.slug, "author": e.author, "id": e.id, "desc": e.description,
                        "source": source(e), "date": date(e), "dated": e.submitted_at.is_some(),
                        "score": e.score_wu, "mean": e.mean_wu.map(|m| m.round() as u64),
                        "max": e.max_wu, "program": e.program_bytes, "witness": e.witness_bytes,
                        "padding": e.padding_bytes.unwrap_or(0),
                        "cost": e.cost_mwu.or(e.max_cost_mwu),
                        "tx": signet_tx(root, &ledger, e).map(|txid| {
                            format!("{}/tx/{txid}?expand", explorer.trim_end_matches('/'))
                        }),
                    })
                })
                .collect();
            let failing: Vec<Value> = entries
                .iter()
                .filter(|e| !e.passed)
                .map(|e| {
                    json!({
                        "slug": e.slug, "author": e.author, "source": source(e),
                        "problems": e.failures.len(),
                        "examples": e.failures.iter().take(3).collect::<Vec<_>>(),
                    })
                })
                .collect();
            let ideal = entries.iter().find_map(|e| e.ideal_witness_bytes);
            json!({
                "category": category, "scheme": scheme(&category), "ideal": ideal,
                "entries": passing, "failing": failing,
            })
        })
        .collect();

    let data = json!({ "categories": categories, "tested": day(tested_at), "qs": qs });
    // Safe inside <script>: no "</" can close the tag early.
    let data = serde_json::to_string(&data)?.replace("</", "<\\/");
    let repo = source_base
        .trim_end_matches('/')
        .split("/tree/")
        .next()
        .unwrap_or(source_base);

    std::fs::create_dir_all(out)?;
    std::fs::write(
        out.join("index.html"),
        PAGE.replace("__DATA__", &data).replace("__REPO__", repo),
    )?;
    eprintln!("wrote {}", out.join("index.html").display());
    Ok(())
}

/// `YYYY-MM-DD` for a Unix time, in UTC.
pub fn day(unix: u64) -> String {
    let days = (unix / 86_400) as i64;
    // Howard Hinnant's civil-from-days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

const PAGE: &str = include_str!("site.html");

#[cfg(test)]
mod tests {
    #[test]
    fn day_formats_unix_time_in_utc() {
        assert_eq!(super::day(0), "1970-01-01");
        assert_eq!(super::day(1_791_550_689), "2026-10-09");
    }
}
