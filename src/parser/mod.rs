mod common;
mod v5;
mod v6;

use crate::model::content::*;
use common::*;

use anyhow::Result;
use std::fs::read;

pub fn parse(s: ParserInput) -> ParserResult<(Version, Page)> {
    let (s, version) = header(s)?;
    println!("parsed header version {version:?}");

    let (s, page) = match version {
        Version::V3 => panic!("can't handle v3"),
        Version::V5 => v5::read_page_v5(s)?,
        Version::V6 => v6::read_page_v6(s)?,
    };

    Ok((s, (version, page)))
}

pub fn parse_notebook(notebook: crate::model::fs::Notebook) -> Result<Notebook> {
    println!("parsing notebook: {notebook:?}");
    let mut pages = Vec::new();

    for page in notebook.pages.iter() {
        println!("processing page: {}", page.id);
        let page_path = notebook.root.join(format!("{}.rm", page.id));

        let contents = match read(&page_path) {
            Ok(contents) => contents,
            Err(_e) => {
                println!("failed to open file at {page_path:?}");
                continue;
            }
        };

        match parse(&contents) {
            Ok((_, (version, page))) => {
                println!("Parsed successfully: {version:?}");
                pages.push(page);
            }
            Err(e) => {
                // let f: String = e;
                // println!("Failed to parse: {:?}", e);
                println!("Failed to parse {:?}: {}", &notebook.name, e);
            }
        }
    }

    Ok(Notebook { pages })
}
