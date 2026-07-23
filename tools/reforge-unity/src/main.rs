use std::path::PathBuf;

use anyhow::Result;
use clap::{Args, Parser, Subcommand, ValueEnum};
use reforge_output::{OutputFormat, write_report};
use reforge_unity_engine::{AnalyzeOptions, Config};

#[derive(Parser)]
#[command(
    name = "reforge-unity",
    version,
    about = "Run the experimental Unity specialization"
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Analyze a Unity project explicitly.
    Analyze(Analyze),
    /// List Unity rules and their observation contracts.
    Rules(Rules),
}

#[derive(Args)]
struct Rules {
    #[arg(long, value_enum, default_value_t = RulesFormat::Human)]
    output: RulesFormat,
}

#[derive(Args)]
struct Analyze {
    #[arg(default_value = ".")]
    path: PathBuf,
    #[arg(long, value_enum, default_value_t = Format::Human)]
    output: Format,
    #[arg(long)]
    reproducible: bool,
}

#[derive(Clone, Copy, ValueEnum)]
enum Format {
    Human,
    Json,
    Yaml,
    Html,
    Sarif,
}

#[derive(Clone, Copy, ValueEnum)]
enum RulesFormat {
    Human,
    Json,
}

fn main() -> Result<()> {
    match Cli::parse().command {
        Command::Analyze(command) => {
            let report = reforge_unity_engine::analyze(&AnalyzeOptions {
                root: command.path,
                config: Config::default(),
                reproducible: command.reproducible,
            })?;
            write_report(
                std::io::stdout().lock(),
                &report,
                match command.output {
                    Format::Human => OutputFormat::Human,
                    Format::Json => OutputFormat::Json,
                    Format::Yaml => OutputFormat::Yaml,
                    Format::Html => OutputFormat::Html,
                    Format::Sarif => OutputFormat::Sarif,
                },
            )
        }
        Command::Rules(command) => {
            let rules = reforge_unity_engine::rules();
            match command.output {
                RulesFormat::Json => println!("{}", serde_json::to_string_pretty(&rules)?),
                RulesFormat::Human => {
                    for rule in rules {
                        println!(
                            "{}\t{}\t{}",
                            rule["rule"].as_str().unwrap_or_default(),
                            rule["subject"].as_str().unwrap_or_default(),
                            rule["description"].as_str().unwrap_or_default()
                        );
                    }
                }
            }
            Ok(())
        }
    }
}
