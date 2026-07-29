use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(about = "Offline, auditable Reforge rule calibration")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Subcommand)]
pub(crate) enum Command {
    Packet {
        #[arg(long)]
        candidates: PathBuf,
        #[arg(long)]
        output: PathBuf,
    },
    Validate {
        #[arg(long)]
        packet: PathBuf,
        #[arg(long)]
        labels: PathBuf,
    },
    Summarize {
        #[arg(long)]
        packet: PathBuf,
        #[arg(long)]
        reviewer_a: PathBuf,
        #[arg(long)]
        reviewer_b: PathBuf,
        #[arg(long)]
        adjudication: Option<PathBuf>,
        #[arg(long)]
        corpus_digest: String,
        #[arg(long)]
        report_digest: String,
        #[arg(long)]
        output: PathBuf,
    },
    Corpus {
        #[command(subcommand)]
        command: CorpusCommand,
    },
    VerifyReports(VerifyReportsArgs),
    VerifyPromotion {
        #[arg(long)]
        corpus: PathBuf,
        #[arg(long)]
        audit: Vec<PathBuf>,
        #[arg(long)]
        summary: Vec<PathBuf>,
    },
}

#[derive(Subcommand)]
pub(crate) enum CorpusCommand {
    Validate {
        #[arg(long)]
        manifest: PathBuf,
    },
    Matrix {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        output: Option<PathBuf>,
    },
}

#[derive(clap::Args)]
pub(crate) struct VerifyReportsArgs {
    #[arg(long)]
    pub(crate) manifest: PathBuf,
    #[arg(long)]
    pub(crate) repository: String,
    #[arg(long)]
    pub(crate) revision: String,
    #[arg(long)]
    pub(crate) codebase: PathBuf,
    #[arg(long)]
    pub(crate) codebase_repeat: PathBuf,
    #[arg(long)]
    pub(crate) dataflow: PathBuf,
    #[arg(long)]
    pub(crate) dataflow_repeat: PathBuf,
    #[arg(long)]
    pub(crate) combined: PathBuf,
    #[arg(long)]
    pub(crate) combined_repeat: PathBuf,
    #[arg(long)]
    pub(crate) metrics: PathBuf,
    #[arg(long)]
    pub(crate) metrics_repeat: PathBuf,
    #[arg(long)]
    pub(crate) flow_ir: PathBuf,
    #[arg(long)]
    pub(crate) flow_ir_repeat: PathBuf,
    #[arg(long)]
    pub(crate) output: PathBuf,
}
