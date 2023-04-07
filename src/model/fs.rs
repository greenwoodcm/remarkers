use std::path::PathBuf;

#[derive(Debug)]
pub struct Notebooks {
    pub root: PathBuf,
    pub notebooks: Vec<Notebook>,
}

#[derive(Debug)]
pub struct Notebook {
    pub name: String,
    pub root: PathBuf,
    pub pages: Vec<Page>,
}

#[derive(Debug)]
pub struct Page {
    pub id: String,
}

pub mod serde {
    use serde::Deserialize;

    #[derive(Debug, Deserialize)]
    pub struct NotebookMetadata {
        #[serde(rename = "visibleName")]
        pub visible_name: String,
        #[serde(rename = "type")]
        pub element_type: ElementType,
    }

    #[derive(Debug, Deserialize, PartialEq)]
    pub enum ElementType {
        DocumentType,
        CollectionType,
    }

    #[derive(Debug)]
    pub struct NotebookContent {
        pub pages: Option<Vec<String>>,
    }

    impl From<NotebookContentRaw> for NotebookContent {
        fn from(value: NotebookContentRaw) -> Self {
            let pages = value.pages.or_else(|| {
                value
                    .c_pages
                    .and_then(|v| v.pages)
                    .map(|pages| pages.into_iter().map(|p| p.id).collect())
            });

            NotebookContent { pages }
        }
    }

    #[derive(Debug, Deserialize)]
    pub struct NotebookContentRaw {
        #[serde(rename = "cPages")]
        c_pages: Option<CPages>,
        pages: Option<Vec<String>>,
    }

    #[derive(Debug, Deserialize)]
    pub struct CPages {
        pages: Option<Vec<CPagesPage>>,
    }

    #[derive(Debug, Deserialize)]
    pub struct CPagesPage {
        id: String,
    }
}
