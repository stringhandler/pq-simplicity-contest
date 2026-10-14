//! The pq-contest runner: tests entries offline, and builds the static leaderboard.
//!
//! See `docs/plan.md` for how it fits together, and `docs/rules.md` for what it enforces.

mod entries;
mod lua;
mod results;
mod signer;
mod site;
mod spend;
mod test;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Test pq-contest entries, and build the leaderboard")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Test entries against known and random keys, valid and invalid signatures.
    Test {
        /// The contest repository's root: the folder holding `entries/` and `keys/`.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// Test only this entry, as `entries/<category>/<entry>`.
        #[arg(long)]
        entry: Option<PathBuf>,
        /// The `shrincs-signer` binary (tx-manifest-wallet's `contrib/shrincs-signer`).
        #[arg(long, env = "SHRINCS_SIGNER", default_value = "shrincs-signer")]
        signer: PathBuf,
        /// How many random keys to test, on top of the known ones.
        #[arg(long, default_value_t = 2)]
        random: usize,
        /// Where to write results.
        #[arg(long, default_value = "results")]
        out: PathBuf,
        /// Skip entries that already have a result for their current `verifier.simf`.
        #[arg(long)]
        new_only: bool,
    },
    /// Put each new passing entry on the Simplicity signet, once: lock a coin to its
    /// program and spend it. Entries already in `onchain.json` are skipped.
    Spend {
        #[arg(long, default_value = ".")]
        root: PathBuf,
        #[arg(long, default_value = "results")]
        results: PathBuf,
        #[arg(long, env = "SHRINCS_SIGNER", default_value = "shrincs-signer")]
        signer: PathBuf,
        /// A tx-manifest-wallet wallet file for bitcoin-signet, funded, that pays for the
        /// locks and receives the spends.
        #[arg(long)]
        wallet: PathBuf,
        /// The Simplicity signet's Esplora API.
        #[arg(
            long,
            default_value = "https://signet.simplicity-lang.org/explorer/api"
        )]
        esplora: String,
        /// Sats to lock per entry. The spend's fee comes out of it, and the fee estimate
        /// sizes the witness for the program's costliest branch, so leave room.
        #[arg(long, default_value_t = 30_000)]
        lock_sat: u64,
        /// Fee rate in sat/vB, for the lock and the spend.
        #[arg(long, default_value_t = 1.0)]
        fee_rate: f32,
    },
    /// Build the static leaderboard from a results folder.
    Site {
        #[arg(long, default_value = "results")]
        results: PathBuf,
        #[arg(long, default_value = "public")]
        out: PathBuf,
        /// Where entry sources are browsable; an entry's path is appended.
        #[arg(
            long,
            default_value = "https://github.com/stringhandler/pq-contest-site/tree/main"
        )]
        source_base: String,
        /// The contest repository's root, for reading each entry's current `entry.json`.
        #[arg(long, default_value = ".")]
        root: PathBuf,
        /// The signet explorer; `/tx/<txid>` is appended.
        #[arg(long, default_value = "https://signet.simplicity-lang.org/explorer")]
        explorer: String,
    },
}

fn main() -> anyhow::Result<()> {
    match Cli::parse().command {
        Command::Test {
            root,
            entry,
            signer,
            random,
            out,
            new_only,
        } => test::run(&root, entry.as_deref(), &signer, random, &out, new_only),
        Command::Spend {
            root,
            results,
            signer,
            wallet,
            esplora,
            lock_sat,
            fee_rate,
        } => spend::run(&spend::Options {
            root: &root,
            results: &results,
            signer: &signer,
            wallet: &wallet,
            esplora: &esplora,
            lock_sat,
            fee_rate,
        }),
        Command::Site {
            results,
            out,
            source_base,
            root,
            explorer,
        } => site::build(&results, &out, &source_base, &root, &explorer),
    }
}
