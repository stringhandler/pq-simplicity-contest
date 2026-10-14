//! Finding entries: `entries/<category>/<entry>/` with `entry.json`, `verifier.simf` and
//! `convert.lua`.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// The categories the runner can test.
pub const CATEGORIES: &[&str] = &["shrincs-l-stateful"];

#[derive(Debug, Clone, Deserialize)]
pub struct Meta {
    pub name: String,
    pub author: String,
    pub description: String,
    #[serde(default)]
    pub source: Option<String>,
    /// A signet transaction that spent a coin locked to this entry, as a txid. Optional,
    /// self-reported and unchecked.
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
    /// The entry id: the first 8 bytes of the verifier's SHA-256, in hex.
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
        let category_dir = dir.parent().context("an entry folder has a category")?;
        let category = file_name(category_dir)?;
        for folder in [dir.as_path(), category_dir] {
            if std::fs::symlink_metadata(folder)
                .with_context(|| format!("cannot read {}", folder.display()))?
                .is_symlink()
            {
                bail!(
                    "{} is a link: entries must be real folders",
                    folder.display()
                );
            }
        }
        for file in ["entry.json", "verifier.simf", "convert.lua"] {
            own_file(&dir.join(file))?;
        }
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
        if !CATEGORIES.contains(&name.as_str()) {
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

/// Refuses `path` unless it's a regular file, or missing (the caller reports that where it
/// reads it). An entry may read only what it contains: a link could point anywhere on the
/// machine testing it, and errors quote what they fail to parse.
fn own_file(path: &Path) -> Result<()> {
    match std::fs::symlink_metadata(path) {
        Ok(m) if m.is_file() => Ok(()),
        Ok(_) => bail!(
            "{} must be a regular file, not a link or a folder",
            path.display()
        ),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e).with_context(|| format!("cannot read {}", path.display())),
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch contest root with one entry, `entries/shrincs-l-stateful/e`.
    fn root(name: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!("pq-entries-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let dir = root.join("entries/shrincs-l-stateful/e");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("entry.json"),
            r#"{"name": "e", "author": "a", "description": "d"}"#,
        )
        .unwrap();
        std::fs::write(dir.join("verifier.simf"), "fn main() {}").unwrap();
        std::fs::write(
            dir.join("convert.lua"),
            "function convert(sig) return {} end",
        )
        .unwrap();
        root
    }

    #[test]
    fn loads_regular_files() {
        let root = root("ok");
        assert_eq!(discover(&root).unwrap().len(), 1);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn refuses_links() {
        for file in ["entry.json", "verifier.simf", "convert.lua"] {
            let root = root("file-link");
            let path = root.join("entries/shrincs-l-stateful/e").join(file);
            let target = root.join("secret");
            std::fs::rename(&path, &target).unwrap();
            std::os::unix::fs::symlink(&target, &path).unwrap();
            let err = discover(&root).unwrap_err().to_string();
            assert!(err.contains("must be a regular file"), "{file}: {err}");
            std::fs::remove_dir_all(root).unwrap();
        }

        let root = root("dir-link");
        let real = root.join("elsewhere");
        std::fs::rename(root.join("entries/shrincs-l-stateful/e"), &real).unwrap();
        std::os::unix::fs::symlink(&real, root.join("entries/shrincs-l-stateful/e")).unwrap();
        let err = discover(&root).unwrap_err().to_string();
        assert!(err.contains("must be real folders"), "{err}");
        std::fs::remove_dir_all(root).unwrap();
    }
}
