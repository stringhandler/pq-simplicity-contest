//! `runner spend`: put each new passing entry on the Simplicity signet, once.
//!
//! For an entry that passes its tests and isn't on chain yet, this locks a small coin to
//! the entry's program under a fresh key, then spends it: `shrincs-signer` signs the
//! spend's `sig_all_hash`, the entry's `convert.lua` maps the signature onto its
//! witnesses, and the wallet's own finalize path builds the witness stack.
//!
//! The ledger, `onchain.json` at the contest root, records every step. An entry already
//! in it, with the same verifier, is never spent again. A lock that went out but whose
//! spend didn't is resumed on the next run, rather than locked a second time.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use lwk_wollet::elements::bitcoin::{Amount, OutPoint, ScriptBuf, TxOut, Txid, Witness};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use tx_manifest_lib::bitcoin_backend::EsploraClient;
use tx_manifest_lib::bitcoin_wallet::{BitcoinWallet, Branch};
use tx_manifest_lib::chain::{ChainFamily, Network};
use tx_manifest_lib::covenant::{
    bitcoin_covenant_sighash, bitcoin_node_check, compute_bitcoin_covenant_address,
    estimate_bitcoin_covenant_witness_wu, finalize_bitcoin_covenant_input, CompileOpts,
    ProgramSource,
};
use tx_manifest_lib::psbt_builder::{
    build_psbt, extract_tx, finalize_key_path_inputs, sign_key_path_inputs, BuildPsbtRequest,
    KeyPathSigner, PsbtInput, PsbtOutputSpec,
};

use crate::entries::{self, Entry};
use crate::lua;
use crate::results::EntryResult;
use crate::signer::Signer;

pub const LEDGER: &str = "onchain.json";

/// One entry's on-chain record. Keyed in the ledger by `<category>/<entry>`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Record {
    /// SHA-256 of the `verifier.simf` that was spent. A different one starts over.
    pub sha256: String,
    /// The throwaway key the coin was locked to: its seed, and `PK`. It guards a few
    /// thousand signet sats for a few minutes, so it's kept in the open.
    pub seed: String,
    pub pk: String,
    pub lock_txid: Option<String>,
    pub spend_txid: Option<String>,
    /// The spend's witness weight on chain, in WU.
    pub spend_witness_wu: Option<usize>,
    pub date: Option<String>,
}

pub type Ledger = BTreeMap<String, Record>;

pub fn load_ledger(root: &Path) -> Result<Ledger> {
    match std::fs::read_to_string(root.join(LEDGER)) {
        Ok(text) => serde_json::from_str(&text).with_context(|| format!("{LEDGER} is not valid")),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Ledger::new()),
        Err(e) => Err(e).with_context(|| format!("cannot read {LEDGER}")),
    }
}

fn save_ledger(root: &Path, ledger: &Ledger) -> Result<()> {
    let tmp = root.join(format!("{LEDGER}.tmp"));
    std::fs::write(&tmp, serde_json::to_string_pretty(ledger)? + "\n")?;
    std::fs::rename(&tmp, root.join(LEDGER))?;
    Ok(())
}

pub struct Options<'a> {
    pub root: &'a Path,
    pub results: &'a Path,
    pub signer: &'a Path,
    pub wallet: &'a Path,
    pub esplora: &'a str,
    pub lock_sat: u64,
    pub fee_rate: f32,
}

pub fn run(opts: &Options) -> Result<()> {
    let wallet_file = tx_manifest_lib::wallet::load_wallet(opts.wallet)?;
    if wallet_file.network() != Some(Network::BitcoinSignet) {
        bail!(
            "{} is a {} wallet; `spend` needs a bitcoin-signet one",
            opts.wallet.display(),
            wallet_file.network
        );
    }
    let wallet = BitcoinWallet::from_mnemonic(&wallet_file.mnemonic, Network::BitcoinSignet)?;
    let esplora = EsploraClient::new(opts.esplora);
    let signer = Signer::new(opts.signer);
    let mut ledger = load_ledger(opts.root)?;
    let key_dir = opts.results.join(".keys");
    std::fs::create_dir_all(&key_dir)?;

    for entry in entries::discover(opts.root)? {
        let key = format!("{}/{}", entry.category, entry.slug);
        if entry.meta.signet_tx.is_some() {
            eprintln!("-- {key}: has a signet_tx in entry.json; skipping");
            continue;
        }
        if ledger
            .get(&key)
            .is_some_and(|r| r.sha256 == entry.sha256 && r.spend_txid.is_some())
        {
            eprintln!("-- {key}: already on signet; skipping");
            continue;
        }
        let path = opts
            .results
            .join(&entry.category)
            .join(format!("{}.json", entry.slug));
        let passed = std::fs::read_to_string(&path)
            .ok()
            .and_then(|t| serde_json::from_str::<EntryResult>(&t).ok())
            .is_some_and(|r| r.passed && r.sha256 == entry.sha256);
        if !passed {
            eprintln!("-- {key}: no passing result for this verifier; run `test` first");
            continue;
        }

        eprintln!("== {key}");
        match spend_entry(
            &entry,
            &key,
            &mut ledger,
            &wallet,
            &esplora,
            &signer,
            &key_dir,
            opts,
        ) {
            Ok(txid) => eprintln!("   on signet: {txid}"),
            Err(e) => eprintln!("   FAILED: {e:#}"),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn spend_entry(
    entry: &Entry,
    key: &str,
    ledger: &mut Ledger,
    wallet: &BitcoinWallet,
    esplora: &EsploraClient,
    signer: &Signer,
    key_dir: &Path,
    opts: &Options,
) -> Result<Txid> {
    let program = ProgramSource::read_unpinned(entry.verifier())?;
    let script = std::fs::read_to_string(entry.converter())?;

    // A fresh key, unless a lock for this same verifier is already out.
    let record = ledger.entry(key.to_string()).or_default();
    if record.sha256 != entry.sha256 {
        let mut seed = [0u8; 48];
        rand::rngs::OsRng.fill_bytes(&mut seed);
        *record = Record {
            sha256: entry.sha256.clone(),
            seed: hex::encode(seed),
            ..Record::default()
        };
    }
    let seed_path: PathBuf = key_dir.join(format!("onchain-{}.seed", entry.slug));
    std::fs::write(&seed_path, format!("{}\n", record.seed))?;
    record.pk = signer.pubkey(&seed_path)?;
    let params = HashMap::from([("PK".to_string(), record.pk.clone())]);
    let hints = HashMap::from([("PK".to_string(), "bytes32".to_string())]);
    let address = compute_bitcoin_covenant_address(
        &program,
        &params,
        &hints,
        &[],
        Network::BitcoinSignet,
        bitcoin(),
    )?;
    let script_pubkey = address.script_pubkey();
    eprintln!("   address {address}");

    // 1. Lock, or find the lock that is already out.
    let (outpoint, amount) = match record.lock_txid.clone() {
        Some(txid) => {
            let txid: Txid = txid.parse()?;
            let tx = esplora.transaction(txid)?;
            let vout = tx
                .output
                .iter()
                .position(|o| o.script_pubkey == script_pubkey)
                .context("the recorded lock pays nothing to this entry's address")?;
            eprintln!("   resuming lock {txid}");
            (
                OutPoint {
                    txid,
                    vout: vout as u32,
                },
                tx.output[vout].value.to_sat(),
            )
        }
        None => {
            let (txid, vout) = lock(wallet, esplora, &script_pubkey, opts)?;
            record.lock_txid = Some(txid.to_string());
            save_ledger(opts.root, ledger)?;
            eprintln!("   locked {} sat in {txid}", opts.lock_sat);
            (OutPoint { txid, vout }, opts.lock_sat)
        }
    };

    // 2. Spend it back to the wallet.
    let witness_wu =
        estimate_bitcoin_covenant_witness_wu(&program, &params, &hints, &[], bitcoin())?;
    let (back, _) = esplora.next_unused(wallet, Branch::Receive)?;
    let req = BuildPsbtRequest {
        inputs: vec![PsbtInput::Covenant {
            input_id: "entry".into(),
            outpoint,
            script_pubkey: script_pubkey.clone(),
            amount,
            sequence: None,
            witness_wu: Some(witness_wu),
        }],
        outputs: vec![],
        fee_rate: opts.fee_rate,
        change_script: Some(back.script_pubkey()),
        lock_time: None,
    };
    let mut built = build_psbt(&req).context("build the spend")?;
    let unsigned = built.psbt.unsigned_tx.clone();
    let prevouts = vec![TxOut {
        value: Amount::from_sat(amount),
        script_pubkey,
    }];
    let msg = bitcoin_covenant_sighash(
        &program,
        &params,
        &hints,
        &[],
        &unsigned,
        &prevouts,
        0,
        bitcoin(),
    )?;
    let sig = signer.sign_raw(&seed_path, 1, &msg)?;
    let witnesses: serde_json::Map<String, serde_json::Value> = lua::convert(&script, &sig)?
        .into_iter()
        .map(|(name, value)| {
            (
                name,
                serde_json::json!({ "type": "simplicityhl", "value": value }),
            )
        })
        .collect();
    let stack = finalize_bitcoin_covenant_input(
        &program,
        &params,
        &hints,
        &[],
        Some(&serde_json::Value::Object(witnesses)),
        None,
        &unsigned,
        &prevouts,
        0,
        bitcoin(),
    )
    .context("satisfy the entry's program")?;
    bitcoin_node_check(&stack).map_err(|why| anyhow::anyhow!("the node would reject it: {why}"))?;
    let witness = Witness::from_slice(&stack);
    let wu = witness.size();
    built.psbt.inputs[0].final_script_witness = Some(witness);
    let txid = esplora.broadcast(&extract_tx(built.psbt)?)?;

    let record = ledger.get_mut(key).expect("recorded above");
    record.spend_txid = Some(txid.to_string());
    record.spend_witness_wu = Some(wu);
    record.date = Some(today());
    save_ledger(opts.root, ledger)?;
    Ok(txid)
}

/// Lock `opts.lock_sat` to `script_pubkey` from one wallet coin, returning the outpoint.
fn lock(
    wallet: &BitcoinWallet,
    esplora: &EsploraClient,
    script_pubkey: &ScriptBuf,
    opts: &Options,
) -> Result<(Txid, u32)> {
    let mut coins = esplora.scan(wallet, 20)?;
    coins.retain(|u| u.value >= opts.lock_sat + 5_000);
    // Confirmed coins first, then the smallest that is big enough.
    coins.sort_by_key(|u| (u.height.is_none(), u.value));
    let coin = coins.first().with_context(|| {
        format!(
            "the wallet has no coin of at least {} sat; fund it from the signet faucet",
            opts.lock_sat + 5_000
        )
    })?;
    let (change, _) = esplora.next_unused(wallet, Branch::Change)?;
    let req = BuildPsbtRequest {
        inputs: vec![PsbtInput::Wallet {
            input_id: "funding".into(),
            outpoint: coin.outpoint,
            witness_utxo: coin.txout(),
            sequence: None,
        }],
        outputs: vec![PsbtOutputSpec {
            script_pubkey: script_pubkey.clone(),
            amount: opts.lock_sat,
        }],
        fee_rate: opts.fee_rate,
        change_script: Some(change.script_pubkey()),
        lock_time: None,
    };
    let mut built = build_psbt(&req).context("build the lock")?;
    let plan = [KeyPathSigner {
        input_index: 0,
        branch: coin.branch,
        index: coin.index,
    }];
    sign_key_path_inputs(&mut built.psbt, wallet, &plan)?;
    finalize_key_path_inputs(&mut built.psbt)?;
    let tx = extract_tx(built.psbt)?;
    let vout = tx
        .output
        .iter()
        .position(|o| &o.script_pubkey == script_pubkey)
        .expect("the lock pays the entry") as u32;
    Ok((esplora.broadcast(&tx)?, vout))
}

fn bitcoin() -> CompileOpts {
    CompileOpts {
        family: ChainFamily::Bitcoin,
        ..CompileOpts::default()
    }
}

fn today() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    crate::site::day(secs)
}
