mod config;
mod context;
mod datatype;
mod processor;

use crate::datatype::FileMeta;
use clap::{arg, command, Parser};
use context::PatternsContext;
use futures::StreamExt;
use mongodb::bson::doc;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use crate::context::{AsyncIOContext, MongoDBContext, OperatingMode, ThreadMotorContext};
use crate::processor::{Directory, Processor, Exec};

pub struct MaidSweeper {
    context: Arc<ThreadMotorContext>,
}

#[derive(Parser, Debug)]
#[command(version, about = "Call the maid sweeper", long_about=None)]
struct Args {
    /// The operating mode. Can be either "tag" or "sweep".
    mode: OperatingMode,
    /// If set, the program will store the metadata in a MongoDB database when sweeping.
    #[arg(
        long,
        default_value = "false",
        help = "Whether or not to print debug outputs"
    )]
    debug: bool,

    #[arg(
        long,
        short = 'd',
        default_value = "false",
        help = "Whether or not to use MongoDB"
    )]
    use_mongodb: bool,

    #[arg(
        long,
        default_value = "maid-sweeper",
        help = "The name of the MongoDB database to use."
    )]
    database_name: String,

    #[arg(
        long,
        default_value = "mongodb://localhost:27017",
        help = "The host of the MongoDB server."
    )]
    mongodb_host: String,

    /// The path to the patterns configuration file. By default it is ~/.maidsweeprs.yaml.
    #[arg(short = 'c', long = "config")]
    config_file: Option<String>,

    #[arg(
        short = 't',
        long = "tag",
        value_name = "TAG",
        help = "The tags to filter when sweeping, if not specified, all tags will be considered when storing info or cleaning."
    )]
    tags: Option<Vec<String>>,

    /// The paths to scan and label.
    #[arg(required = false,
        num_args = 1..,
        default_value = ".", 
        value_name = "PATH")]
    paths: Option<Vec<PathBuf>>,

    /// The command to execute. Like in fd -x or find -exec, you can use {} to represent the path.
    #[arg(
        short = 'x',
        long = "exec",
        num_args = 1..,
        allow_hyphen_values = true,
        value_terminator = ";"
    )]
    exec_args: Option<Vec<OsString>>,
}

impl MaidSweeper {
    async fn dispatch<P, T>(
        processor: P,
        context: Arc<ThreadMotorContext>,
        file_meta: FileMeta,
    ) -> () where P: Processor<ThreadMotorContext, T> + 'static {
        match processor.process(context.clone(), file_meta).await {
            Ok(_) => (),
            Err(e) => {
                eprintln!("Error: {}", e);
            }
        }
    }

    fn sweep(
        &self,
        paths: Option<Vec<PathBuf>>,
        tags: Option<Vec<String>>,
    ) -> impl Iterator<Item = tokio::task::JoinHandle<()>> + '_ {
        // expand synonyms
        let mut new_tags: Vec<String> = Vec::new();
        if let Some(tags) = tags {
            for tag in tags.into_iter() {
                if let Some(synonyms) = self.context.get_patterns().synonyms.get(&tag) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(tag);
                }
            }
        }

        let paths = paths.unwrap_or(vec![PathBuf::from(".")]);
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

    async fn mongodb_sweep(
        &self,
        _paths: Option<Vec<PathBuf>>,
        tags: Option<Vec<String>>,
    ) -> Result<(), Box<dyn Error>> {
        let mut new_tags: Vec<String> = Vec::new();
        if let Some(tags) = tags {
            for keyword in tags.into_iter() {
                if let Some(synonyms) = self.context.get_patterns().synonyms.get(&keyword) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(keyword);
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
                        Exec {},
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

    async fn run(args: Args) -> Result<(), Box<dyn Error>> {
        let maid: MaidSweeper = Self {
            context: Arc::new(
                ThreadMotorContext::new(
                    args.mode,
                    args.use_mongodb,
                    &args.database_name,
                    &args.mongodb_host,
                    args.debug,
                    args.config_file,
                    args.exec_args,
                )
                .await?,
            ),
        };

        if args.mode == OperatingMode::Sweep || !args.use_mongodb {
            let tasks = maid.sweep(args.paths, args.tags);
            futures::future::join_all(tasks).await;
        } else {
            maid.mongodb_sweep(args.paths, args.tags).await?;
        }
        Ok(())
    }
}

#[tokio::main]
pub async fn main() {
    let args = Args::parse();
    if args.debug {
        println!("{:?}", args);
    }
    return match MaidSweeper::run(args).await {
        Ok(_) => (),
        Err(e) => println!("Error: {}", e),
    };
}
