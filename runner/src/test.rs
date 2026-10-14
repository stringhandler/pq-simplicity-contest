//! `runner test`: every entry against known and random keys, valid and invalid
//! signatures. See `docs/rules.md`, "How entries are tested".

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use lwk_wollet::elements::bitcoin::{
    absolute::LockTime, transaction::Version, Amount, OutPoint, ScriptBuf, Sequence, Transaction,
    TxIn, TxOut, Witness,
};
use rand::RngCore;
use tx_manifest_lib::chain::ChainFamily;
use tx_manifest_lib::covenant::{
    bitcoin_covenant_sighash, bitcoin_node_check, finalize_bitcoin_covenant_input, CompileOpts,
    ProgramSource,
};

use crate::entries::{self, Entry};
use crate::lua;
use crate::results::{Case, EntryResult, RunInfo};
use crate::signer::{RawSig, Signer, TAMPERED_FIELDS};

/// The leaf indexes tested for each key: the first few, a middle one, the deepest
/// path (206) and the special last leaf (207).
const QS: &[u32] = &[1, 2, 3, 10, 100, 206, 207];

const COMPILER: &str = "SimplicityHL 0.7.1 (delta1/SimplicityHL, bitcoin branch), Bitcoin jets";
const SIGNER: &str = "shrincs-cpp fde3603fb4fe268a6095ca90bae3ae651fc69ff7 (SHRINCS_L)";

struct Key {
    label: String,
    seed: PathBuf,
    known: bool,
}

pub fn run(
    root: &Path,
    only: Option<&Path>,
    signer: &Path,
    random: usize,
    out: &Path,
    new_only: bool,
) -> Result<()> {
    let signer = Signer::new(signer);
    let mut entries = match only {
        Some(dir) => vec![Entry::load(root, dir)?],
        None => entries::discover(root)?,
    };
    if new_only {
        // An entry whose result was made from this same verifier has nothing new to test.
        entries.retain(|e| {
            let tested =
                std::fs::read_to_string(out.join(&e.category).join(format!("{}.json", e.slug)))
                    .ok()
                    .and_then(|t| serde_json::from_str::<EntryResult>(&t).ok())
                    .is_some_and(|r| r.sha256 == e.sha256);
            if tested {
                eprintln!(
                    "-- {}: unchanged since its last test; skipping",
                    e.rel_path.display()
                );
            }
            !tested
        });
        if entries.is_empty() {
            eprintln!("No new or changed entries to test.");
            return Ok(());
        }
    }

    // Known keys, then random ones whose seeds go into the results.
    let mut keys = Vec::new();
    let mut known = Vec::new();
    let mut known_paths: Vec<PathBuf> = std::fs::read_dir(root.join("keys"))
        .context("no keys/ folder")?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|x| x == "seed"))
        .collect();
    known_paths.sort();
    for seed in known_paths {
        let label = seed
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
        known.push(label.clone());
        keys.push(Key {
            label,
            seed,
            known: true,
        });
    }
    let key_dir = out.join(".keys");
    std::fs::create_dir_all(&key_dir)?;
    let mut random_seeds = Vec::new();
    for i in 1..=random {
        let mut seed = [0u8; 48];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        let path = key_dir.join(format!("random-{i}.seed"));
        std::fs::write(&path, format!("{}\n", hex::encode(seed)))?;
        random_seeds.push(hex::encode(seed));
        keys.push(Key {
            label: format!("random-{i}"),
            seed: path,
            known: false,
        });
    }
    anyhow::ensure!(
        keys.len() >= 2,
        "need at least two keys, for the other-key test"
    );

    let run = RunInfo {
        tested_at: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        qs: QS.to_vec(),
        known_keys: known,
        random_seeds,
        compiler: COMPILER.into(),
        signer: SIGNER.into(),
    };

    for entry in &entries {
        eprintln!("== {}", entry.rel_path.display());
        let result = test_entry(entry, &keys, &signer, &run);
        let dir = out.join(&entry.category);
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join(format!("{}.json", entry.slug)),
            serde_json::to_string_pretty(&result)? + "\n",
        )?;
        if result.passed {
            eprintln!(
                "   passed: score {} WU",
                result.score_wu.unwrap_or_default()
            );
        } else {
            eprintln!("   FAILED:");
            for f in &result.failures {
                eprintln!("   - {f}");
            }
        }
    }
    Ok(())
}

/// What became of one signature.
enum Outcome {
    /// The node would accept the spend.
    Accepted { stack: Vec<Vec<u8>>, cost_mwu: u64 },
    /// The program, or the node's rules, rejected it.
    Rejected(String),
    /// The converter failed, so the program never saw it.
    ConverterFailed(String),
}

fn test_entry(entry: &Entry, keys: &[Key], signer: &Signer, run: &RunInfo) -> EntryResult {
    let mut failures = Vec::new();
    let mut cases = Vec::new();
    let mut q1_stack: Option<Vec<Vec<u8>>> = None;

    let loaded = (|| -> Result<(ProgramSource, String)> {
        let program = ProgramSource::read_unpinned(entry.verifier())?;
        let script = std::fs::read_to_string(entry.converter())
            .with_context(|| format!("no {}", entry.converter().display()))?;
        Ok((program, script))
    })();
    let (program, script) = match loaded {
        Ok(x) => x,
        Err(e) => return result(entry, vec![format!("{e:#}")], vec![], None, run),
    };

    let (tx, prevouts) = test_spend();
    for (i, key) in keys.iter().enumerate() {
        let attempt = (|| -> Result<()> {
            let pk = signer.pubkey(&key.seed)?;
            let params = HashMap::from([("PK".to_string(), pk.clone())]);
            let hints = HashMap::from([("PK".to_string(), "bytes32".to_string())]);
            let msg = bitcoin_covenant_sighash(
                &program,
                &params,
                &hints,
                &[],
                &tx,
                &prevouts,
                0,
                bitcoin(),
            )
            .with_context(|| format!("does not compile with PK = {pk}"))?;
            let spend =
                |sig: &RawSig| spend_with(&program, &params, &hints, &script, sig, &tx, &prevouts);

            // Every valid signature must verify.
            for &q in QS {
                let sig = signer.sign_raw(&key.seed, q, &msg)?;
                match spend(&sig) {
                    Outcome::Accepted { stack, cost_mwu } => {
                        if q == 1 && key.known && q1_stack.is_none() {
                            q1_stack = Some(stack.clone());
                        }
                        cases.push(Case {
                            key: key.label.clone(),
                            q,
                            wu: Witness::from_slice(&stack).size(),
                            cost_mwu,
                            sig_bytes: sig.raw.len() / 2,
                        });
                    }
                    Outcome::Rejected(why) | Outcome::ConverterFailed(why) => failures.push(
                        format!("{}, q = {q}: a valid signature failed: {why}", key.label),
                    ),
                }
            }

            // Every invalid one must be rejected, by the program and not by the converter.
            let other = &keys[(i + 1) % keys.len()];
            let mut wrong_msg = msg;
            wrong_msg[31] ^= 0x01;
            let base = signer.sign_raw(&key.seed, 1, &msg)?;
            let mut invalid = vec![
                (
                    format!("a signature by {}", other.label),
                    signer.sign_raw(&other.seed, 1, &msg)?,
                ),
                (
                    "a signature over another message".to_string(),
                    signer.sign_raw(&key.seed, 1, &wrong_msg)?,
                ),
            ];
            for field in TAMPERED_FIELDS {
                invalid.push((
                    format!("a signature with `{field}` tampered"),
                    base.tampered(field),
                ));
            }
            for (what, sig) in invalid {
                match spend(&sig) {
                    Outcome::Rejected(_) => {}
                    Outcome::Accepted { .. } => {
                        failures.push(format!("{}: accepted {what}", key.label))
                    }
                    Outcome::ConverterFailed(why) => failures.push(format!(
                        "{}: convert.lua failed on {what} ({why}). A converter must map every \
                         signature; rejecting is the program's job",
                        key.label
                    )),
                }
            }
            Ok(())
        })();
        if let Err(e) = attempt {
            failures.push(format!("{}: {e:#}", key.label));
        }
    }
    result(entry, failures, cases, q1_stack, run)
}

fn spend_with(
    program: &ProgramSource,
    params: &HashMap<String, String>,
    hints: &HashMap<String, String>,
    script: &str,
    sig: &RawSig,
    tx: &Transaction,
    prevouts: &[TxOut],
) -> Outcome {
    let values = match lua::convert(script, sig) {
        Ok(v) => v,
        Err(e) => return Outcome::ConverterFailed(format!("{e:#}")),
    };
    let witnesses: serde_json::Map<String, serde_json::Value> = values
        .into_iter()
        .map(|(name, value)| {
            (
                name,
                serde_json::json!({ "type": "simplicityhl", "value": value }),
            )
        })
        .collect();
    let stack = match finalize_bitcoin_covenant_input(
        program,
        params,
        hints,
        &[],
        Some(&serde_json::Value::Object(witnesses)),
        None,
        tx,
        prevouts,
        0,
        bitcoin(),
    ) {
        Ok(stack) => stack,
        Err(e) => return Outcome::Rejected(format!("{e:#}")),
    };
    match bitcoin_node_check(&stack) {
        Ok(cost) => Outcome::Accepted {
            stack,
            cost_mwu: milliweight(&format!("{cost:?}")),
        },
        Err(why) => Outcome::Rejected(why),
    }
}

fn result(
    entry: &Entry,
    failures: Vec<String>,
    cases: Vec<Case>,
    q1_stack: Option<Vec<Vec<u8>>>,
    run: &RunInfo,
) -> EntryResult {
    let passed = failures.is_empty() && !cases.is_empty();
    let known_q1: Vec<usize> = cases
        .iter()
        .filter(|c| c.q == 1 && run.known_keys.contains(&c.key))
        .map(|c| c.wu)
        .collect();
    let score = known_q1.iter().copied().max();
    let mean = (!cases.is_empty())
        .then(|| cases.iter().map(|c| c.wu as f64).sum::<f64>() / cases.len() as f64);
    let item = |from_end: usize| {
        q1_stack
            .as_ref()
            .and_then(|s| s.len().checked_sub(from_end).map(|i| s[i].len()))
    };
    EntryResult {
        category: entry.category.clone(),
        slug: entry.slug.clone(),
        path: entry.rel_path.to_string_lossy().into_owned(),
        id: entry.id().into(),
        sha256: entry.sha256.clone(),
        name: entry.meta.name.clone(),
        author: entry.meta.author.clone(),
        description: entry.meta.description.clone(),
        source: entry.meta.source.clone(),
        submitted_at: entry.submitted_at.clone(),
        passed,
        failures,
        score_wu: if passed { score } else { None },
        mean_wu: mean,
        max_wu: cases.iter().map(|c| c.wu).max(),
        program_bytes: item(3),
        witness_bytes: item(4),
        padding_bytes: q1_stack
            .as_ref()
            .filter(|s| s.len() == 5)
            .map(|s| s[0].len()),
        max_cost_mwu: cases.iter().map(|c| c.cost_mwu).max(),
        cost_mwu: cases
            .iter()
            .find(|c| c.q == 1 && run.known_keys.contains(&c.key))
            .map(|c| c.cost_mwu),
        ideal_witness_bytes: cases.iter().find(|c| c.q == 1).map(|c| c.sig_bytes),
        cases,
        run: run.clone(),
    }
}

fn bitcoin() -> CompileOpts {
    CompileOpts {
        family: ChainFamily::Bitcoin,
        ..CompileOpts::default()
    }
}

/// `Cost`'s milli-weight, from its `Debug` form (`Cost(18816530)`): it has no accessor.
fn milliweight(debug: &str) -> u64 {
    debug
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0)
}

/// The test transaction: one input, one output. Programs read only its `sig_all_hash`
/// (which commits to the leaf being spent, so each key's hash differs), and every entry
/// sees the same transaction.
fn test_spend() -> (Transaction, Vec<TxOut>) {
    let tx = Transaction {
        version: Version::TWO,
        lock_time: LockTime::ZERO,
        input: vec![TxIn {
            previous_output: OutPoint {
                txid: format!("{:064x}", 0x9c_u8).parse().expect("txid"),
                vout: 0,
            },
            script_sig: ScriptBuf::new(),
            sequence: Sequence::MAX,
            witness: Witness::new(),
        }],
        output: vec![TxOut {
            value: Amount::from_sat(90_000),
            script_pubkey: ScriptBuf::from_bytes([&[0x51, 0x20][..], &[0x42; 32]].concat()),
        }],
    };
    let prevout = TxOut {
        value: Amount::from_sat(100_000),
        script_pubkey: ScriptBuf::from_bytes([&[0x51, 0x20][..], &[0x07; 32]].concat()),
    };
    (tx, vec![prevout])
}
