use std::{error::Error, path::{Path, PathBuf}, str::FromStr};
use mongodb::{options::ClientOptions, Client};
use dirs;
use crate::config;

#[derive(Debug, Clone, Copy)]
pub enum OperatingMode {
    Tag,
    Sweep,
}

impl FromStr for OperatingMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tag" => Ok(OperatingMode::Tag),
            "sweep" => Ok(OperatingMode::Sweep),
            _ => Err(format!("{} is not a valid operating mode", s)),
        }
    }
}

pub trait AsyncIOContext {
    fn is_debug(&self) -> bool;
}

pub trait MongoDBContext {
    fn use_mongodb(&self) -> bool;
    fn get_db(&self) -> mongodb::Database;
}

pub trait PatternsContext {
    fn get_patterns(&self) -> &config::Patterns;
}
pub trait MaidContext: PatternsContext + AsyncIOContext + MongoDBContext + Send + Sync + 'static {
    fn get_exec_args(&self) -> Option<Vec<std::ffi::OsString>>;
    fn get_operating_mode(&self) -> OperatingMode;
}

pub struct ThreadMotorContext {
    client: Option<Client>,
    database_name: String,
    debug: bool,
    patterns: config::Patterns,
    exec_args: Option<Vec<std::ffi::OsString>>,
    operating_mode: OperatingMode,
}

impl ThreadMotorContext {
    pub async fn new<T>(
        operating_mode: OperatingMode,
        use_mongodb: bool,
        database_name: &str,
        host: &str,
        debug: bool,
        config_file: Option<T>,
        exec_args: Option<Vec<std::ffi::OsString>>,
    ) -> Result<Self, Box<dyn Error>>
    where
        T: AsRef<Path> + Clone,
    {
        let options: ClientOptions = ClientOptions::parse_async(format!("{}", host)).await?;
        let client = if use_mongodb {
            Some(Client::with_options(options)?)
        } else {
            None
        };
        let patterns = if let Some(path) = config_file {
            config::load_patterns(path)
        } else {
            config::load_patterns(PathBuf::from(dirs::home_dir().unwrap().join(".maidsweep.yaml")))
        };

        Ok(ThreadMotorContext {
            operating_mode: operating_mode,
            client: client,
            database_name: database_name.to_string(),
            debug: debug,
            patterns: patterns,
            exec_args: exec_args,
        })
    }
}

impl AsyncIOContext for ThreadMotorContext {
    fn is_debug(&self) -> bool {
        self.debug
    }
}

impl MongoDBContext for ThreadMotorContext {
    fn use_mongodb(&self) -> bool {
        self.client.is_some()
    }
    fn get_db(&self) -> mongodb::Database {
        self.client.as_ref().unwrap().database(&self.database_name)
    }
}

impl PatternsContext for ThreadMotorContext {
    fn get_patterns(&self) -> &config::Patterns {
        &self.patterns
    }
}

unsafe impl Send for ThreadMotorContext {}
unsafe impl Sync for ThreadMotorContext {}

impl MaidContext for ThreadMotorContext {
    fn get_exec_args(&self) -> Option<Vec<std::ffi::OsString>> {
        self.exec_args.clone()
    }

    fn get_operating_mode(&self) -> OperatingMode {
        self.operating_mode
    }
}
