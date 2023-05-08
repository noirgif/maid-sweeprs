use serde::{Deserialize, Serialize};

/// Define a type that models our metadata.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub(crate) struct Item {
    pub path: String,
    pub tags: Vec<String>,
}