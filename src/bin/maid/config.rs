use once_cell::sync::Lazy;
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
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
    T: AsRef<Path> + Clone,
{
    let patterns_yaml = std::fs::read_to_string::<T>(config_path.clone());
    if patterns_yaml.is_err() {
        eprintln!(
            "Error: Could not find patterns file at {}.",
            config_path.as_ref().display()
        );
        std::process::exit(1);
    }
    let patterns: Result<PatternsYamlSchema, serde_yaml::Error> =
        serde_yaml::from_str(&patterns_yaml.unwrap());
    if let Err(err) = patterns {
        eprintln!("{}", err.to_string());

        std::process::exit(1);
    }
    let patterns = patterns.unwrap();

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

static SHELLS : Lazy<[String; 6]> = Lazy::new(|| [
    env::var("SHELL").unwrap_or_default(),
    env::var("COMSPEC").unwrap_or_default(),
    "/bin/zsh".to_owned(),
    "/bin/bash".to_owned(),
    "/bin/ash".to_owned(),
    "/bin/sh".to_owned(),
]);

pub fn find_shell() -> Option<(String, String)> {

    let shell: Option<String> = SHELLS
        .iter()
        .filter(|shell| Path::new(shell).exists())
        .next()
        .and_then(|s| Some(s.to_owned()));

    let  arg1  = 
    
    if env::var("COMSPEC").unwrap_or_default().eq(shell.clone().unwrap_or_default().as_str())   {
        "/c"
    } else {
        "-c"
    };

    if shell.is_some() {
        Some((shell.unwrap(), arg1.to_owned()))
    } else {
        None
    }
}