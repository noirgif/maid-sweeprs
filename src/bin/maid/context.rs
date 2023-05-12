use std::{error::Error, path::{Path, PathBuf}, str::FromStr};
use mongodb::{options::ClientOptions, Client};
use dirs;
use crate::config::{self, MaidConfig};


pub trait AsyncIOContext {
    fn is_debug(&self) -> bool;
}

pub trait PatternsContext {
    fn get_patterns(&self) -> &config::Patterns;
}

pub trait MaidContext: AsyncIOContext + PatternsContext + Sync + Send + 'static {
    fn get_config(&self) -> &MaidConfig;
}

pub struct SimpleContext {
    config: MaidConfig,
    patterns: config::Patterns,
}

impl AsyncIOContext for SimpleContext {
    fn is_debug(&self) -> bool {
        self.config.debug
    }
}

impl PatternsContext for SimpleContext {
    fn get_patterns(&self) -> &config::Patterns {
        &self.patterns
    }
}

impl SimpleContext {
    pub fn new(config: MaidConfig) -> Self {
        let patterns = if let Some(path) = config.config_file {
            config::load_patterns(path)
        } else {
            config::load_patterns(PathBuf::from(dirs::home_dir().unwrap().join(".maidsweep.yaml")))
        };
        SimpleContext { config, patterns }
    }
}

impl MaidContext for SimpleContext {
    fn get_config(&self) -> &MaidConfig {
        &self.config
    }
}

pub struct MongoDBContext {
    simp_context: SimpleContext,
    client: Client,
    database: mongodb::Database,
}

impl MongoDBContext {
    pub async fn new(
       config: MaidConfig, 
    ) -> Result<Self, Box<dyn Error>>
    {
        let options: ClientOptions = ClientOptions::parse_async(format!("{}", config.mongodb_host)).await?;
        let client = Client::with_options(options)?;
        let database = client.database("maidsweep");
        let simp_context = SimpleContext::new(config);

        Ok(MongoDBContext {
           client, simp_context, database})
    }

    pub fn get_db(&self) -> mongodb::Database {
        self.database
    }
}

impl AsyncIOContext for MongoDBContext {
    fn is_debug(&self) -> bool {
        self.simp_context.is_debug()
    }
}

impl PatternsContext for MongoDBContext {
    fn get_patterns(&self) -> &config::Patterns {
        self.simp_context.get_patterns()
    }
}



unsafe impl Send for MongoDBContext {}
unsafe impl Sync for MongoDBContext {}

impl MaidContext for MongoDBContext {
    fn get_config(&self) -> &MaidConfig {
        self.simp_context.get_config()
    }
}