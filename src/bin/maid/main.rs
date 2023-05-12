mod config;
mod context;
mod datatype;
mod processor;

use crate::datatype::FileMeta;
use clap::Parser;
use config::MaidConfig;
use context::{MaidContext, PatternsContext, SimpleContext};
use futures::StreamExt;
use mongodb::bson::doc;
use std::error::Error;

use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use crate::context::{AsyncIOContext, MongoDBContext};
use crate::processor::{Directory, Exec, Processor};

pub struct MaidSweeper<C>
where
    C: MaidContext,
{
    context: Arc<C>,
}

impl<C> MaidSweeper<C>
where
    C: MaidContext,
{
    async fn dispatch<P, T>(processor: P, context: Arc<C>, file_meta: FileMeta) -> ()
    where
        P: Processor<C, T> + 'static,
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
                if let Some(synonyms) = self.context.get_patterns().synonyms.get(tag) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(tag.to_string());
                }
            }
        }

        let paths = self
            .context
            .get_config()
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
}

impl MaidSweeper<MongoDBContext> {
    async fn mongodb_sweep(&self) -> Result<(), Box<dyn Error>> {
        let mut new_tags: Vec<String> = Vec::new();
        if let Some(tags) = &self.context.get_config().tags {
            for keyword in tags.into_iter() {
                if let Some(synonyms) = self.context.get_patterns().synonyms.get(keyword) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(keyword.to_string());
                }
            }
        }
        let mut cursor = self
            .context
            .get_db()
            .collection::<datatype::FileMetaCompat>(processor::COLLECTION_NAME)
            .find(doc! {"tags": {"$in": new_tags}}, None)
            .await?;

        let mut tasks: Vec<tokio::task::JoinHandle<()>> = Vec::new();
        while let Some(item) = cursor.next().await {
            match item {
                Ok(item) => {
                    tasks.push(tokio::spawn(Self::dispatch(
                        Exec::new(&self.context),
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
    if !config.use_mongodb && !config.save {
        let maid = MaidSweeper {
            context: Arc::new(SimpleContext::new(config)),
        };
        let tasks = maid.sweep();
        futures::future::join_all(tasks).await;
    } else {
        let maid = MaidSweeper {
            context: Arc::new(MongoDBContext::new(config).await?),
        };
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
