use crate::config::{self, MaidConfig};
use dirs;
use mongodb::{options::ClientOptions, Client};
use std::{error::Error, path::PathBuf};

pub struct MongoDBContext {
    pub client: Client,
    pub database: mongodb::Database,
}

impl MongoDBContext {
    pub async fn new(config: &MaidConfig) -> Result<Self, Box<dyn Error>> {
        let options: ClientOptions =
            ClientOptions::parse(format!("{}", config.mongodb_host)).await?;
        let client = Client::with_options(options)?;
        let database = client.database("maidsweep");

        Ok(MongoDBContext { client, database })
    }

    pub fn get_db(&self) -> &mongodb::Database {
        &self.database
    }
}

pub struct MaidContext {
    pub config: MaidConfig,
    pub patterns: config::Patterns,
    pub mongodb: Option<MongoDBContext>,
}

impl MaidContext {
    pub fn is_debug(&self) -> bool {
        self.get_config().debug
    }

    pub fn get_config(&self) -> &MaidConfig {
        &self.config
    }

    pub fn get_db(&self) -> Option<&mongodb::Database> {
        if let Some(ref mongodb) = self.mongodb {
            Some(mongodb.get_db())
        } else {
            None
        }
    }

    pub async fn new(config: MaidConfig) -> Self {
        let patterns = if let Some(ref path) = config.config_file {
            config::load_patterns(path)
        } else {
            config::load_patterns(PathBuf::from(
                dirs::home_dir().unwrap().join(".maidsweep.yaml"),
            ))
        };

        let mongodb = if config.use_mongodb || config.save {
            Some(MongoDBContext::new(&config).await.unwrap())
        } else {
            None
        };

        MaidContext {
            config,
            mongodb,
            patterns,
        }
    }
}

unsafe impl Send for MaidContext {}
unsafe impl Sync for MaidContext {}
