mod config;
mod context;
mod datatype;
mod processor;

use crate::datatype::FileMeta;
use clap::Parser;
use config::MaidConfig;
use context::MaidContext;
use futures::StreamExt;
use mongodb::bson::doc;
use std::{error::Error, io};

use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use crate::processor::{Directory, Exec, Processor};

pub struct MaidSweeper
{
    context: Arc<MaidContext>,
}

impl MaidSweeper
{
    async fn dispatch<P, T>(processor: P, context: Arc<MaidContext>, file_meta: FileMeta) -> ()
    where
        P: Processor<T> + 'static,
    {
        match processor.process(context.clone(), file_meta).await {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    fn sweep(&self) -> impl Iterator<Item = tokio::task::JoinHandle<()>> + '_ {
        // expand synonyms
        let mut new_tags: Vec<String> = Vec::new();
        if let Some(tags) = &self.context.get_config().tags {
            for tag in tags.into_iter() {
                if let Some(synonyms) = self.context.patterns.synonyms.get(tag) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(tag.to_string());
                }
            }
        }

        let paths = self
            .context
            .config
            .paths
            .as_ref()
            .and_then(|paths| Some(paths.iter().map(|path| path.to_owned()).collect()))
            .unwrap_or(vec![PathBuf::from(".")]);
        if self.context.is_debug() {
            println!("Tagging {:?}", paths);
        }

        // map to paths
        paths.into_iter().map(move |path|
            // can fork, as different directories are independent
            tokio::spawn(Self::dispatch(
                Directory {},
                self.context.clone(),
                FileMeta {
                    path: path.to_owned(),
                    tags: Some(new_tags.clone()),
                    last_modified: None,
                },
            )))
    }
    async fn mongodb_sweep(&self) -> Result<(), Box<dyn Error>> {
        let mut new_tags: Vec<String> = Vec::new();
        let database = if let Some(db) = self.context.get_db() {
            db
        } else {
            return Err(Box::new(io::Error::new(io::ErrorKind::NotFound, "Database not found")));
        };


        if let Some(tags) = &self.context.get_config().tags {
            for keyword in tags.into_iter() {
                if let Some(synonyms) = self.context.patterns.synonyms.get(keyword) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(keyword.to_string());
                }
            }
        }

        let mut cursor = database
            .collection::<datatype::FileMetaCompat>(processor::COLLECTION_NAME)
            .find(doc! {"tags": {"$in": new_tags}}, None)
            .await?;

        let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        while let Some(item) = cursor.next().await {
            match item {
                Ok(item) => {
                    tasks.push(tokio::spawn(Self::dispatch(
                        Exec{},
                        self.context.clone(),
                        FileMeta {
                            path: PathBuf::from(&item.path.clone()),
                            tags: Some(item.tags),
                            last_modified: Some(item.last_modified),
                        },
                    )));
                }
                Err(item) => {
                    println!("Error obtaining data from database: {:?}", item);
                    return Err(Box::new(item));
                }
            }
        }
        for task in tasks {
            if let Err(e) = task.await {
                eprintln!("Error: {}", e);
            }
        }
        Ok(())
    }
}

async fn run(config: MaidConfig) -> Result<(), Box<dyn Error>> {
    let maid = MaidSweeper {
        context: Arc::new(MaidContext::new(config).await),
    };
    if !maid.context.get_config().use_mongodb {
        let tasks = maid.sweep();
        futures::future::join_all(tasks).await;
    } else {
        maid.mongodb_sweep().await?;
    }
    Ok(())
}

#[tokio::main]
pub async fn main() {
    let config = MaidConfig::parse();
    if config.debug {
        println!("{:?}", config);
    }
    return match run(config).await {
        Ok(_) => (),
        Err(e) => println!("Error: {}", e),
    };
}
