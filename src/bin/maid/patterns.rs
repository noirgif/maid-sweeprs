use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::string::String;

#[derive(Debug, Deserialize)]
struct PatternsYamlSchema {
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

#[derive(Debug)]
pub struct Patterns {
    pub typical_files_re: HashMap<String, Vec<Regex>>,
    pub filenames_re: Vec<(Vec<String>, Regex)>,
    pub extensions: HashMap<String, HashSet<String>>,
    pub synonyms: HashMap<String, HashSet<String>>,
}

pub fn load_patterns<T>(config_path: T) -> Patterns
where
    T: AsRef<Path>,
{
    let patterns_yaml = std::fs::read_to_string::<T>(config_path).unwrap();
    let patterns: PatternsYamlSchema = serde_yaml::from_str(&patterns_yaml).unwrap();

    let typical_files_re = patterns
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
        .collect();
    let filenames_re = patterns
        .filenames
        .into_iter()
        .map(|filename_pattern| {
            (
                filename_pattern.tags,
                Regex::new(&filename_pattern.pattern).unwrap(),
            )
        })
        .collect();
    let extensions = patterns
        .extensions
        .into_iter()
        .map(|(key, values)| (key, values.into_iter().collect::<HashSet<_>>()))
        .collect();

    let synonyms = patterns
        .synonyms
        .into_iter()
        .map(|(key, values)| (key, values.into_iter().collect::<HashSet<_>>()))
        .collect();

    Patterns {
        typical_files_re,
        filenames_re,
        extensions,
        synonyms,
    }
}
