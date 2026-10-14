//! Signatures from `shrincs-signer`, and the tampered copies the negative tests use.

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use serde::Deserialize;

pub struct Signer {
    bin: PathBuf,
}

/// One stateful SHRINCS-L signature, as `shrincs-signer sign-raw` prints it and as a
/// converter receives it. All byte strings are lowercase hex.
#[derive(Debug, Clone, Deserialize)]
pub struct RawSig {
    pub q: u32,
    pub msg: String,
    pub pk: String,
    pub pk_seed: String,
    pub pk_root: String,
    pub raw: String,
    pub sl_root: String,
    pub r: String,
    pub counter: String,
    pub chains: Vec<String>,
    pub path: Vec<String>,
}

/// The fields a negative test tampers with.
pub const TAMPERED_FIELDS: &[&str] = &[
    "sl_root",
    "r",
    "counter",
    "chains[0]",
    "chains[63]",
    "path[0]",
    "path[last]",
];

impl Signer {
    pub fn new(bin: &Path) -> Self {
        Self {
            bin: bin.to_path_buf(),
        }
    }

    fn call(&self, args: &[&str]) -> Result<String> {
        let out = Command::new(&self.bin)
            .args(args)
            .output()
            .with_context(|| {
                format!(
                    "cannot run {}; build it with `make -C contrib/shrincs-signer` in \
                     tx-manifest-wallet, and pass --signer or set SHRINCS_SIGNER",
                    self.bin.display()
                )
            })?;
        if !out.status.success() {
            bail!(
                "shrincs-signer {}: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8(out.stdout)?.trim().to_string())
    }

    /// `PK.seed || PK.root`, as `0x`-prefixed hex.
    pub fn pubkey(&self, seed: &Path) -> Result<String> {
        self.call(&["pubkey", "--seed", &seed.to_string_lossy()])
    }

    pub fn sign_raw(&self, seed: &Path, q: u32, msg: &[u8; 32]) -> Result<RawSig> {
        let json = self.call(&[
            "sign-raw",
            "--seed",
            &seed.to_string_lossy(),
            "--q",
            &q.to_string(),
            "--msg",
            &hex::encode(msg),
        ])?;
        serde_json::from_str(&json).context("parse shrincs-signer sign-raw output")
    }
}

impl RawSig {
    /// A copy with one bit flipped in the first byte of `field`, `raw` kept consistent.
    pub fn tampered(&self, field: &str) -> RawSig {
        let mut s = self.clone();
        let target: &mut String = match field {
            "sl_root" => &mut s.sl_root,
            "r" => &mut s.r,
            "counter" => &mut s.counter,
            "chains[0]" => &mut s.chains[0],
            "chains[63]" => &mut s.chains[63],
            "path[0]" => &mut s.path[0],
            "path[last]" => s.path.last_mut().expect("a stateful signature has a path"),
            other => panic!("no field {other}"),
        };
        let first = u8::from_str_radix(&target[..2], 16).expect("hex") ^ 0x01;
        target.replace_range(..2, &format!("{first:02x}"));
        s.raw = [
            s.sl_root.as_str(),
            &s.r,
            &s.counter,
            &s.chains.concat(),
            &s.path.concat(),
        ]
        .concat();
        s
    }
}
