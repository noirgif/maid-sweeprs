mod context;
mod data;
mod dispatcher;
mod patterns;

use crate::data::FileMeta;
use clap::{arg, command, Parser, Subcommand};
use context::PatternsContext;
use futures::StreamExt;
use mongodb::bson::doc;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::Arc;
use std::vec;

use crate::context::{AsyncIOContext, MongoDBContext, ThreadMotorContext};
use crate::dispatcher::{Directory, Dispatcher, Exec};

pub struct MaidSweeper {
    context: Arc<ThreadMotorContext>,
}

#[derive(Parser, Debug)]
#[command(version, about = "Call the maid sweeper", long_about=None)]
struct Args {
    #[arg(
        long,
        default_value = "12",
        help = "The maximum number of workers to use."
    )]
    max_workers: usize,
    #[arg(
        long,
        default_value = "maid-sweeper",
        help = "The name of the database to use."
    )]
    database_name: String,
    #[arg(
        long,
        default_value = "mongodb://localhost:27017",
        help = "The host of the MongoDB server."
    )]
    mongodb_host: String,
    /// The path to the patterns configuration file. By default it is ~/.maid-sweeprs/patterns.yaml.
    #[arg(short = 'c', long = "config")]
    config_file: Option<String>,
    /// Enable debug output.
    #[arg(long)]
    debug: bool,
    #[command(subcommand)]
    subcommand: SubCommand,
}

#[derive(Debug, Subcommand)]
enum SubCommand {
    /// scan the paths and tag the files wherein
    #[command(arg_required_else_help = true)]
    Tag {
        /// The paths to scan and label.
        #[arg(required = true, value_name = "PATH")]
        paths: Vec<PathBuf>,
        /// The command to execute. Like in fd -x or find -exec, you can use {} to represent the path.
        #[arg(
            short = 'x',
            long = "exec",
            allow_hyphen_values = true,
            value_terminator = ";"
        )]
        exec_args: Option<Vec<OsString>>,
    },
    /// sweep the files with the specified tags
    Sweep {
        /// The tags to sweep. If multiple tags are specified, they are treated as OR conditions.
        #[arg(short = 't', long = "tag", required = true)]
        tags: Vec<String>,
        /// The command to execute. Like in fd -x or find -exec, you can use {} to represent the path.
        #[arg(
            short = 'x',
            long = "exec",
            allow_hyphen_values = true,
            value_terminator = ";"
        )]
        exec_args: Option<Vec<OsString>>,
    },
}

impl MaidSweeper {
    async fn run(args: Args) -> Result<(), Box<dyn Error>> {
        match args.subcommand {
            SubCommand::Tag { paths, exec_args } => {
                let maid = Self {
                    context: Arc::new(
                        ThreadMotorContext::new(
                            &args.database_name,
                            &args.mongodb_host,
                            args.debug,
                            args.config_file,
                            exec_args,
                        )
                        .await?,
                    ),
                };
                let mut tasks = Vec::new();

                for path in paths.iter() {
                    if maid.context.is_debug() {
                        println!("Tagging {:?}", path);
                    }
                    tasks.push(Directory.dispatch(
                        maid.context.clone(),
                        FileMeta {
                            path: path.to_owned(),
                            tags: None,
                            last_modified: None,
                        },
                    ));
                }

                futures::future::join_all(tasks).await;
            }
            SubCommand::Sweep { tags, exec_args } => {
                let maid = Self {
                    context: Arc::new(
                        ThreadMotorContext::new(
                            &args.database_name,
                            &args.mongodb_host,
                            args.debug,
                            args.config_file,
                            exec_args,
                        )
                        .await?,
                    ),
                };

                let mut new_args = vec![];
                for keyword in tags.iter() {
                    if let Some(synonyms) = maid.context.get_patterns().synonyms.get(keyword) {
                        new_args.extend(synonyms);
                    } else {
                        new_args.push(keyword);
                    }
                }

                let mut cursor = maid
                    .context
                    .get_db()
                    .collection::<data::FileMetaCompat>(dispatcher::COLLECTION_NAME)
                    .find(doc! {"tags": {"$in": new_args}}, None)
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
    match MaidSweeper::run(matches).await {
        Ok(_) => (),
        Err(e) => println!("Error: {}", e),
    }
}
