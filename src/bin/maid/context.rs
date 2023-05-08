use std::{error::Error, path::Path};

use mongodb::{options::ClientOptions, Client};

use crate::patterns;

pub trait AsyncIOContext {
    fn is_debug(&self) -> bool;
}

//}

pub trait MongoDBContext {
    fn get_db(&self) -> mongodb::Database;
}

pub trait PatternsContext {
    fn get_patterns(&self) -> &patterns::Patterns;
}

pub struct ThreadMotorContext {
    client: Client,
    database_name: String,
    debug: bool,
    patterns: patterns::Patterns,
}

pub trait MaidContext: PatternsContext + AsyncIOContext + MongoDBContext + Sync {}

impl ThreadMotorContext {
    pub async fn new<T>(
        database_name: &str,
        host: &str,
        debug: bool,
        config_file: Option<T>,
    ) -> Result<Self, Box<dyn Error>>
    where
        T: AsRef<Path>,
    {
        let options: ClientOptions = ClientOptions::parse_async(format!("{}", host)).await?;
        let client = Client::with_options(options)?;
        let patterns = if let Some(path) = config_file {
            patterns::load_patterns(path)
        } else {
            patterns::load_patterns("~/.config/maid-sweeprs/patterns.yaml")
        };

        Ok(ThreadMotorContext {
            client: client,
            database_name: database_name.to_string(),
            debug: debug,
            patterns: patterns,
        })
    }
}

impl AsyncIOContext for ThreadMotorContext {
    fn is_debug(&self) -> bool {
        self.debug
    }
}

impl MongoDBContext for ThreadMotorContext {
    fn get_db(&self) -> mongodb::Database {
        self.client.database(&self.database_name)
    }
}

impl PatternsContext for ThreadMotorContext {
    fn get_patterns(&self) -> &patterns::Patterns {
        &self.patterns
    }
}

unsafe impl Sync for ThreadMotorContext {}

impl MaidContext for ThreadMotorContext {}
