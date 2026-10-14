//! What `runner test` writes, one JSON file per entry, and what `runner site` reads.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntryResult {
    pub category: String,
    pub slug: String,
    /// `entries/<category>/<slug>`.
    pub path: String,
    /// The first 8 bytes of SHA-256 of `verifier.simf`, hex: the on-chain tag's entry id.
    pub id: String,
    pub sha256: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub source: Option<String>,
    /// When the entry was submitted: the date git first committed its `entry.json`
    /// (`YYYY-MM-DD`). `None` for an entry not yet committed.
    #[serde(default)]
    pub submitted_at: Option<String>,

    /// True when every valid signature verified and every invalid one was rejected.
    pub passed: bool,
    pub failures: Vec<String>,

    /// The score: the largest witness stack weight at q = 1 over the known keys, in WU.
    pub score_wu: Option<usize>,
    /// Mean and largest witness stack weight over every valid case.
    pub mean_wu: Option<f64>,
    pub max_wu: Option<usize>,
    /// The q = 1 stack, item by item, for the first known key.
    pub program_bytes: Option<usize>,
    pub witness_bytes: Option<usize>,
    pub padding_bytes: Option<usize>,
    /// The largest program cost bound seen, in milli-weight units.
    pub max_cost_mwu: Option<u64>,
    /// The q = 1 program cost bound, in milli-weight units.
    #[serde(default)]
    pub cost_mwu: Option<u64>,
    /// The ideal witness: the scheme's raw signature at q = 1, as shrincs-cpp produces
    /// it. The same for every entry in a category; `witness_bytes` is how close an entry's
    /// own encoding comes to it.
    #[serde(default)]
    pub ideal_witness_bytes: Option<usize>,

    pub cases: Vec<Case>,
    pub run: RunInfo,
}

/// One valid signature that verified.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Case {
    pub key: String,
    pub q: u32,
    pub wu: usize,
    pub cost_mwu: u64,
    /// The scheme's own signature size: shrincs-cpp's raw signature, before any encoding.
    #[serde(default)]
    pub sig_bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunInfo {
    /// Seconds since the Unix epoch.
    pub tested_at: u64,
    pub qs: Vec<u32>,
    pub known_keys: Vec<String>,
    /// The random keys' seeds, so the run can be replayed.
    pub random_seeds: Vec<String>,
    pub compiler: String,
    pub signer: String,
}
