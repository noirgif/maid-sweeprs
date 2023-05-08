use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::string::String;
use once_cell::sync::Lazy;

#[derive(Debug, Deserialize)]
struct Patterns {
    typical_files: HashMap<String, Vec<String>>,
    extensions: HashMap<String, Vec<String>>,
    filenames: Vec<FilenamePattern>,
    synonyms: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct FilenamePattern {
    tags: Vec<String>,
    pattern: String,
}

pub static TYPICAL_FILES_RE: Lazy<HashMap<String, Vec<Regex>>> = Lazy::new(|| {
    let patterns_yaml = std::fs::read_to_string("patterns.yaml").unwrap();
    let patterns: Patterns = serde_yaml::from_str(&patterns_yaml).unwrap();

    patterns
        .typical_files
        .into_iter()
        .map(|(key, patterns)| {
            (
                key,
                patterns
                    .into_iter()
                    .map(|pattern| Regex::new(&pattern).unwrap())
                    .collect(),
            )
        })
        .collect()
});

pub static FILENAMES_RE: Lazy<Vec<(Vec<String>, Regex)>> = Lazy::new(|| {
    let patterns_yaml = std::fs::read_to_string("patterns.yaml").unwrap();
    let patterns: Patterns = serde_yaml::from_str(&patterns_yaml).unwrap();

    patterns
        .filenames
        .into_iter()
        .map(|filename_pattern| {
            (
                filename_pattern.tags,
                Regex::new(&filename_pattern.pattern).unwrap(),
            )
        })
        .collect()
});

pub static EXTENSIONS: Lazy<HashMap<String, HashSet<String>>> = Lazy::new(|| {
    let patterns_yaml = std::fs::read_to_string("patterns.yaml").unwrap();
    let patterns: Patterns = serde_yaml::from_str(&patterns_yaml).unwrap();

    patterns.extensions.into_iter().map(|(key, values)| {
        (key, values.into_iter().collect::<HashSet<_>>())
    }).collect()
});

pub static SYNONYMS: Lazy<HashMap<String, HashSet<String>>> = Lazy::new(|| {
    let patterns_yaml = std::fs::read_to_string("patterns.yaml").unwrap();
    let patterns: Patterns = serde_yaml::from_str(&patterns_yaml).unwrap();

    patterns.synonyms.into_iter().map(|(key, values)| {
        (key, values.into_iter().collect::<HashSet<_>>())
    }).collect()
});