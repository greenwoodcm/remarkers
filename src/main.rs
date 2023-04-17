use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{filter::Directive, EnvFilter};

use std::path::PathBuf;

mod fs;
mod model;
mod parser;
mod render;
mod sync;

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    #[arg(short, long)]
    log_level: Directive,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Sync {
        #[arg(short, long)]
        dest_dir: PathBuf,
    },
    Convert {
        #[arg(short, long)]
        source_dir: PathBuf,
        #[arg(short, long)]
        output_dir: Option<PathBuf>,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(cli.log_level.clone())
                .from_env_lossy(),
        )
        .init();

    info!("Parsed CLI command: {:?}", cli);

    match cli.command {
        Command::Sync { dest_dir } => {
            sync::sync_remarkable_to_dir(dest_dir)?;
        }
        Command::Convert {
            source_dir,
            output_dir,
        } => {
            let notebooks = fs::scan(source_dir)?;

            let output_dir = output_dir.unwrap_or(PathBuf::from(".").join("output"));
            info!("writing output to directory: {:?}", &output_dir);

            for notebook in notebooks.notebooks {
                let output_path = output_dir.join(format!("{}.pdf", &notebook.name));
                let parsed_notebook = parser::parse_notebook(notebook)?;
                render::render_pdf(parsed_notebook, output_path);
            }
        }
    }

    Ok(())
}
