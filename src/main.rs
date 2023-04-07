use anyhow::Result;
use tracing::Level;
use tracing_subscriber::{filter::Directive, EnvFilter};

use std::env;
use std::path::Path;

mod fs;
mod model;
mod parser;
mod render;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive(Directive::from(Level::TRACE)))
        .init();

    let args: Vec<_> = env::args().collect();
    let root = args.get(1).expect("must provide root as first argument");
    let notebooks = fs::scan(root)?;

    for notebook in notebooks.notebooks {
        let output_path = Path::new(".")
            .join("output")
            .join(format!("{}.pdf", &notebook.name));
        let parsed_notebook = parser::parse_notebook(notebook)?;
        render::render_pdf(parsed_notebook, output_path);
    }

    Ok(())
}
