use clap::{command, Parser};
use once_cell::sync::Lazy;
use regex::{Regex, RegexSet};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
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
    pub typical_files_re: HashMap<String, RegexSet>,
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
        .map(|(key, patterns)| (key, RegexSet::new(patterns).unwrap()))
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

static SHELLS: Lazy<[String; 6]> = Lazy::new(|| {
    [
        env::var("SHELL").unwrap_or_default(),
        env::var("COMSPEC").unwrap_or_default(),
        "/bin/zsh".to_owned(),
        "/bin/bash".to_owned(),
        "/bin/ash".to_owned(),
        "/bin/sh".to_owned(),
    ]
});

pub fn find_shell() -> Option<(String, String)> {
    let shell: Option<String> = SHELLS
        .iter()
        .filter(|shell| Path::new(shell).exists())
        .next()
        .and_then(|s| Some(s.to_owned()));

    let arg1 = if env::var("COMSPEC")
        .unwrap_or_default()
        .eq(shell.clone().unwrap_or_default().as_str())
    {
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

#[derive(Parser, Debug)]
#[command(version, about = "Call the maid sweeper", long_about=None)]
pub struct MaidConfig {
    /// If set, the program will store the metadata in a MongoDB database when sweeping.
    #[arg(
        long,
        default_value = "false",
        help = "Whether or not to print debug outputs"
    )]
    pub debug: bool,

    /// Whether or not to use MongoDB. If false, the program will scan the directories
    #[arg(long, default_value = "false")]
    pub use_mongodb: bool,

    #[arg(
        long,
        default_value = "mongodb://localhost:27017",
        help = "The host of the MongoDB server. It can be used for saving the data, or for loading the data when sweeping."
    )]
    pub mongodb_host: String,

    /// The path to the configuration file. By default it is ~/.maidsweeprs.yaml.
    #[arg(short = 'c', long = "config")]
    pub config_file: Option<String>,

    /// The tags to filter when sweeping, if not specified, all tags will be considered when storing info or cleaning.
    #[arg(short = 't', long = "tag", value_name = "TAG")]
    pub tags: Option<Vec<String>>,

    /// The paths to scan and label. If not specified, the current directory will be used.
    #[arg(required = false,
        num_args = 1..,
        default_value = ".", 
        value_name = "PATH")]
    pub paths: Option<Vec<PathBuf>>,

    /// If set to true, hidden files will be considered when sweeping. For UNIX only.
    #[arg(short = 'H', long = "hidden", default_value = "false")]
    pub hidden: bool,

    /// Can be used to copy files to a directory.
    #[arg(long = "cp", value_name = "PATH")]
    pub copy_to: Option<PathBuf>,

    /// Save the metadata to mongodb.
    #[arg(long = "save", value_name = "MONGODB_URI")]
    pub save: bool,

    /// The command to execute. Like in fd -x or find -exec, you can use {} to represent the path.
    #[arg(
        short = 'x',
        long = "exec",
        num_args = 1..,
        allow_hyphen_values = true,
        value_name = "EXEC_ARG",
        value_terminator = ";"
    )]
    pub exec_args: Option<Vec<OsString>>,

    /// Can be used instead of --exec to move files to a directory.
    #[arg(long = "mv", value_name = "PATH")]
    pub move_to: Option<PathBuf>,

    /// Can be used instead of --exec to delete files.
    #[arg(long = "rm")]
    pub delete: bool,
}
