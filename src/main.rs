use anyhow::Result;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::{filter::Directive, EnvFilter};

use std::{path::PathBuf, str::FromStr};

mod device;
mod fs;
mod model;
mod parser;
mod render;
mod stream;

#[derive(Parser, Debug)]
#[command()]
struct Cli {
    #[arg(short, long)]
    log_level: Option<Directive>,

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
        dest_dir: Option<PathBuf>,
        #[arg(short, long)]
        notebook_filter: Option<String>,
        #[arg(short, long)]
        page_filter: Option<String>,
    },
    Stream {
        /// Enable diagnostics as an overlay, including frame latency and frame rate.
        #[arg(short, long)]
        diagnostics: bool,
    },
    Screengrab {
        #[arg(short, long, default_value = "remarkable-frame.png")]
        dest_file: PathBuf,
    },
}

#[tokio::main]
#[show_image::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(
                    cli.log_level
                        .clone()
                        .unwrap_or_else(|| Directive::from_str("info").unwrap()),
                )
                .from_env_lossy(),
        )
        .init();

    info!("Parsed CLI command: {:?}", cli);

    match cli.command {
        Command::Sync { dest_dir } => {
            let rem = crate::device::Remarkable::open()?;
            rem.rsync_from_device_to(dest_dir)?;
        }
        Command::Convert {
            source_dir,
            dest_dir,
            notebook_filter,
            page_filter,
        } => {
            let notebooks = fs::scan(source_dir)?;

            let dest_dir = dest_dir.unwrap_or(PathBuf::from(".").join("output"));
            info!("writing output to directory: {:?}", &dest_dir);

            for notebook in notebooks.notebooks {
                if let Some(ref notebook_filter) = notebook_filter {
                    if notebook.name != *notebook_filter {
                        continue;
                    }
                }

                let page_range: Box<dyn Fn(usize) -> bool> = match &page_filter {
                    Some(page_filter) if page_filter.contains(":") => {
                        let elems: Vec<_> = page_filter.split(":").collect();
                        let start: usize = elems[0].parse()?;
                        let end: usize = elems[1].parse()?;
                        Box::new(move |p| p >= start && p < end)
                    }
                    Some(page_filter) => {
                        let page_num: usize = page_filter.parse()?;
                        Box::new(move |p| p == page_num)
                    }
                    None => Box::new(|_p| true),
                };

                info!("converting notebook: {}", &notebook.name);
                let output_path = dest_dir.join(format!("{}.pdf", &notebook.name));
                let parsed_notebook = parser::parse_notebook(notebook)?;
                render::render_pdf(parsed_notebook, page_range, output_path);
            }
        }
        Command::Stream { diagnostics } => {
            crate::stream::stream(diagnostics).await.unwrap();
        }
        Command::Screengrab { dest_file } => {
            crate::stream::grab_frame(&dest_file).await?;
        }
    }

    Ok(())
}
