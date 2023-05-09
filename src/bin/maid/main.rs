mod context;
mod data;
mod dispatcher;
mod config;

use crate::data::FileMeta;
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
use crate::dispatcher::{Directory, Dispatcher, Exec};

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

        let mut new_tags: Vec<String> = Vec::new();
        if let Some(tags) = args.tags {
            for keyword in tags.into_iter() {
                if let Some(synonyms) = maid.context.get_patterns().synonyms.get(&keyword) {
                    new_tags.extend(synonyms.iter().map(|s| s.to_owned()));
                } else {
                    new_tags.push(keyword);
                }
            }
        }
        let mut tasks = Vec::new();

        match args.mode {
            OperatingMode::Tag => {
                let paths = args.paths.unwrap_or(vec![PathBuf::from(".")]);
                for path in paths.iter() {
                    if maid.context.is_debug() {
                        println!("Tagging {:?}", path);
                    }
                    tasks.push(Directory.dispatch(
                        maid.context.clone(),
                        FileMeta {
                            path: path.to_owned(),
                            tags: Some(new_tags.clone()),
                            last_modified: None,
                        },
                    ));
                }
                futures::future::join_all(tasks).await;
            }
            OperatingMode::Sweep => {
                let mut cursor = maid
                    .context
                    .get_db()
                    .collection::<data::FileMetaCompat>(dispatcher::COLLECTION_NAME)
                    .find(doc! {"tags": {"$in": new_tags}}, None)
                    .await?;

                let exec = Exec {};
                while let Some(item) = cursor.next().await {
                    match item {
                        Ok(item) => {
                            exec.dispatch(
                                maid.context.clone(),
                                FileMeta {
                                    path: PathBuf::from(&item.path.clone()),
                                    tags: Some(item.tags),
                                    last_modified: Some(item.last_modified),
                                },
                            )
                            .await?;
                        }
                        Err(item) => {
                            println!("Error obtaining data from database: {:?}", item);
                            return Err(Box::new(item));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

#[tokio::main]
pub async fn main() {
    let matches = Args::parse();
    println!("{:?}", matches);
    return match MaidSweeper::run(matches).await {
        Ok(_) => (),
        Err(e) => println!("Error: {}", e),
    };
}
