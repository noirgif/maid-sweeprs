use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Define a type that models our metadata.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct FileMetaCompat {
    pub path: PathBuf,
    pub tags: Vec<String>,
    pub last_modified: u64,
}

/// Define how it is passed around
pub struct FileMeta {
    pub path: PathBuf,
    pub tags: Option<Vec<String>>,
    pub last_modified: Option<u64>,
}

impl Into<FileMetaCompat> for FileMeta {
    fn into(self) -> FileMetaCompat {
        FileMetaCompat {
            path: self.path,
            tags: self.tags.unwrap_or(vec![]),
            last_modified: self.last_modified.unwrap_or(0),
        }
    }
}
