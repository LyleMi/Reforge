use std::io::Write;

use anyhow::Result;
use clap::Parser;

mod cli;
mod corpus;
mod io;
mod model;
mod packet;
mod promotion;
mod reports;
mod statistics;
mod validation;

use cli::{Cli, Command, CorpusCommand};
use io::{read_json, write_json};
use model::{LabelFile, ReviewPacket};

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Packet { candidates, output } => packet::generate_packet(&candidates, &output),
        Command::Validate { packet, labels } => {
            let packet = read_json::<ReviewPacket>(&packet)?;
            let labels = read_json::<LabelFile>(&labels)?;
            validation::validate_labels(&packet, &labels)
        }
        Command::Summarize {
            packet,
            reviewer_a,
            reviewer_b,
            adjudication,
            corpus_digest,
            report_digest,
            output,
        } => statistics::summarize(statistics::SummaryRequest {
            packet: &packet,
            reviewer_a: &reviewer_a,
            reviewer_b: &reviewer_b,
            adjudication: adjudication.as_deref(),
            corpus_digest,
            report_digest,
            output: &output,
        }),
        Command::Corpus { command } => match command {
            CorpusCommand::Validate { manifest } => {
                corpus::load_corpus(&manifest)?;
                writeln!(
                    std::io::stdout(),
                    "valid corpus: {}",
                    corpus::corpus_digest(&manifest)?
                )?;
                Ok(())
            }
            CorpusCommand::Matrix { manifest, output } => {
                let matrix = corpus::corpus_matrix(&corpus::load_corpus(&manifest)?);
                if let Some(output) = output {
                    write_json(&output, &matrix)
                } else {
                    serde_json::to_writer(std::io::stdout(), &matrix)?;
                    writeln!(std::io::stdout())?;
                    Ok(())
                }
            }
        },
        Command::VerifyReports(args) => reports::verify_reports(&args),
        Command::VerifyPromotion {
            corpus,
            audit,
            summary,
        } => {
            let verification = promotion::verify_promotion(&corpus, &audit, &summary)?;
            serde_json::to_writer_pretty(std::io::stdout(), &verification)?;
            writeln!(std::io::stdout())?;
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests;
