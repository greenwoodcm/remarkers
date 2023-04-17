mod common;
mod v5;
mod v6;

use crate::model::content::*;
use common::*;

use anyhow::Result;
use std::fs::read;
use tracing::{error, info, trace};

pub fn parse(s: ParserInput) -> ParserResult<(Version, Vec<Layer>)> {
    let (s, version) = header(s)?;
    trace!("parsed header version {version:?}");

    let (s, page) = match version {
        Version::V3 => panic!("can't handle v3"),
        Version::V5 => v5::read_page_v5(s)?,
        Version::V6 => v6::read_page_v6(s)?,
    };

    Ok((s, (version, page)))
}

pub fn parse_notebook(notebook: crate::model::fs::Notebook) -> Result<Notebook> {
    info!("parsing notebook: {notebook:?}");
    let mut pages = Vec::new();

    for page in notebook.pages.iter() {
        trace!("processing page: {}", page.id);

        let page_path = notebook.root.join(format!("{}.rm", page.id));

        let contents = match read(&page_path) {
            Ok(contents) => contents,
            Err(_e) => {
                error!("failed to open file at {page_path:?}");
                continue;
            }
        };

        match parse(&contents) {
            Ok((_, (version, layers))) => {
                trace!(
                    "Parsed page {} successfully with version {version:?}",
                    page.id
                );
                pages.push(Page {
                    id: page.id.clone(),
                    layers,
                });
            }
            Err(e) => {
                error!("Failed to parse {:?}: {}", &notebook.name, e);
            }
        }
    }

    Ok(Notebook {
        id: notebook.name,
        pages,
    })
}
