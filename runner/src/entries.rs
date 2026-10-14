//! Finding entries: `entries/<category>/<entry>/` with `entry.json`, `verifier.simf` and
//! `convert.lua`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The categories the runner can test, with their tag bytes.
pub const CATEGORIES: &[(&str, u8)] = &[("shrincs-l-stateful", 0x01)];

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub name: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub source: Option<String>,
    /// A signet transaction that spent a coin locked to this entry, as a txid. Optional and
    /// self-reported, until the indexer checks it.
    #[serde(default)]
    pub signet_tx: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Entry {
    pub category: String,
    /// The entry's folder name.
    pub slug: String,
    /// `entries/<category>/<slug>`, relative to the contest root.
    pub rel_path: PathBuf,
    pub dir: PathBuf,
    pub meta: Meta,
    /// SHA-256 of `verifier.simf`, hex.
    pub sha256: String,
    /// The date git first committed this entry's `entry.json`, if it has.
    pub submitted_at: Option<String>,
}

impl Entry {
    /// The entry id: the first 8 bytes of the verifier's SHA-256, as the on-chain tag
    /// carries it.
    pub fn id(&self) -> &str {
        &self.sha256[..16]
    }

    pub fn verifier(&self) -> PathBuf {
        self.dir.join("verifier.simf")
    }

    pub fn converter(&self) -> PathBuf {
        self.dir.join("convert.lua")
    }

    pub fn load(root: &Path, dir: &Path) -> Result<Self> {
        let dir = if dir.is_absolute() {
            dir.to_path_buf()
        } else {
            root.join(dir)
        };
        let slug = file_name(&dir)?;
        let category = file_name(dir.parent().context("an entry folder has a category")?)?;
        let meta: Meta = serde_json::from_str(
            &std::fs::read_to_string(dir.join("entry.json"))
                .with_context(|| format!("{}: no entry.json", dir.display()))?,
        )
        .with_context(|| format!("{}: entry.json", dir.display()))?;
        let source = std::fs::read(dir.join("verifier.simf"))
            .with_context(|| format!("{}: no verifier.simf", dir.display()))?;
        let submitted_at = first_commit_date(&dir.join("entry.json"));
        Ok(Self {
            submitted_at,
            rel_path: PathBuf::from("entries").join(&category).join(&slug),
            category,
            slug,
            dir,
            meta,
            sha256: hex::encode(Sha256::digest(&source)),
        })
    }
}

/// Every entry under `root/entries`, in category and name order. Folders in a category
/// the runner doesn't know are reported and skipped.
pub fn discover(root: &Path) -> Result<Vec<Entry>> {
    let mut out = Vec::new();
    for category in sorted_dirs(&root.join("entries"))? {
        let name = file_name(&category)?;
        if !CATEGORIES.iter().any(|(c, _)| *c == name) {
            eprintln!("skipping entries/{name}: not a category this runner tests");
            continue;
        }
        for dir in sorted_dirs(&category)? {
            if dir.join("entry.json").exists() {
                out.push(Entry::load(root, &dir)?);
            }
        }
    }
    Ok(out)
}

/// The date (`YYYY-MM-DD`) of the commit that added `file`, from git. `None` when it isn't
/// committed, or there's no git: a local, unmerged entry.
fn first_commit_date(file: &Path) -> Option<String> {
    let dir = file.parent()?;
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(["log", "--diff-filter=A", "--format=%cs", "--"])
        .arg(file.file_name()?)
        .output()
        .ok()?;
    let text = String::from_utf8(out.stdout).ok()?;
    // Oldest last: the commit that added it.
    text.lines().last().map(str::to_string)
}

fn sorted_dirs(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(dir)
        .with_context(|| format!("cannot read {}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();
    dirs.sort();
    Ok(dirs)
}

fn file_name(path: &Path) -> Result<String> {
    Ok(path
        .file_name()
        .with_context(|| format!("{} has no name", path.display()))?
        .to_string_lossy()
        .into_owned())
}
