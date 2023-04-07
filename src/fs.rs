//! Scans a directory for notebooks and pages
use std::{ffi::OsStr, fs::File, io::BufReader, path::Path};

use anyhow::{Context, Result};

use crate::model::fs::{
    serde::{ElementType, NotebookContent, NotebookContentRaw, NotebookMetadata},
    Notebook, Notebooks, Page,
};

pub fn scan<T: AsRef<Path>>(root: T) -> Result<Notebooks> {
    let mut notebooks = Vec::new();
    let entries = std::fs::read_dir(root)?;
    for entry in entries {
        let meta_path = match entry {
            Ok(e) => {
                // println!("  path: {}", e.path().display());
                // println!("  path ends with: {}", e.path().as_path().extension().and_then(OsStr::to_str) == Some("metadata"));
                if e.path().as_path().extension().and_then(OsStr::to_str) == Some("metadata") {
                    e.path()
                } else {
                    continue;
                }
            }
            Err(_) => continue,
        };

        let mut dir_path = meta_path.clone();
        dir_path.set_extension("");

        // Open the file in read-only mode with buffer.
        let meta_file = File::open(&meta_path)
            .context(format!("failed to open .metadata file at {meta_path:?}"))?;
        let meta_reader = BufReader::new(meta_file);

        let meta: NotebookMetadata = serde_json::from_reader(meta_reader)?;
        if meta.element_type == ElementType::CollectionType {
            continue;
        }

        // read the associated .content file
        let mut content_path = meta_path.clone();
        content_path.set_extension("content");
        let content_file = File::open(&content_path)
            .context(format!("failed to open .content file at {content_path:?}"))?;
        let content_reader = BufReader::new(content_file);

        let content: NotebookContentRaw = serde_json::from_reader(content_reader)?;
        println!("content raw: {:?}", &content);
        let content: NotebookContent = content.into();

        let pages = match &content.pages {
            Some(pages) => pages.clone(),
            None => {
                // if there's no pages declared in metadata then we assume
                // that there's a single .rm file in the associated directory
                println!("looking for single page in {dir_path:?}");
                std::fs::read_dir(&dir_path)
                    .context(format!("failed to read directory at {dir_path:?}"))?
                    .flat_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().and_then(OsStr::to_str) == Some("rm"))
                    .map(|p| p.to_str().unwrap().to_string())
                    .collect()
            }
        };

        let pages: Vec<_> = pages.into_iter().map(|p| Page { id: p }).collect();

        // println!("found notebook {meta:?} with contents {content:?}, pages {pages:?}");

        notebooks.push(Notebook {
            name: meta.visible_name,
            root: dir_path,
            pages,
        });
    }

    Ok(Notebooks {
        root: "".into(),
        notebooks,
    })
}
