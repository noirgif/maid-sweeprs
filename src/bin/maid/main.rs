mod context;
mod data;
mod dispatcher;
mod patterns;

use clap::{arg, command, Parser, Subcommand};
use context::PatternsContext;
use futures::StreamExt;
use mongodb::bson::doc;
use std::error::Error;
use std::ffi::OsString;
use std::path::PathBuf;
use std::vec;

use crate::context::{AsyncIOContext, MongoDBContext, ThreadMotorContext};
use crate::dispatcher::{Directory, Dispatcher, Exec};

pub struct MaidSweeper {
    context: ThreadMotorContext,
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
        /// The path to the patterns configuration file.
        #[arg(short = 'c', long = "config")]
        config_file: Option<String>,
    },
    /// sweep the files with the specified tags
    Sweep {
        /// The tags to sweep. If multiple tags are specified, they are treated as OR conditions.
        #[arg(short = 't', long = "tag", required = true)]
        tags: Vec<String>,
        /// The command to execute. Like in fd -x or find -exec, you can use {} to represent the path. No \; is required.
        #[arg(short = 'x', long = "exec")]
        exec_args: Vec<OsString>,
    },
}

impl MaidSweeper {
    async fn run(args: Args) -> Result<(), Box<dyn Error>> {
        match args.subcommand {
            SubCommand::Tag { paths, config_file } => {
                let maid = Self {
                    context: ThreadMotorContext::new(
                        &args.database_name,
                        &args.mongodb_host,
                        args.debug,
                        config_file,
                    )
                    .await?,
                };
                let mut tasks = Vec::new();

                for path in paths.iter() {
                    if maid.context.is_debug() {
                        println!("Tagging {:?}", path);
                    }
                    tasks.push(Directory.dispatch(&maid.context, path.to_owned()));
                }

                futures::future::join_all(tasks).await;
            }
            SubCommand::Sweep { tags, exec_args } => {
                let maid = Self {
                    context: ThreadMotorContext::new::<PathBuf>(
                        &args.database_name,
                        &args.mongodb_host,
                        args.debug,
                        None,
                    )
                    .await?,
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
                    .collection::<data::Item>(dispatcher::COLLECTION_NAME)
                    .find(doc! {"tags": {"$in": new_args}}, None)
                    .await?;

                let exec = Exec { args: exec_args };

                while let Some(item) = cursor.next().await {
                    match item {
                        Ok(item) => {
                            match exec
                                .dispatch(&maid.context, PathBuf::from(&item.path.clone()))
                                .await
                            {
                                Ok(_) => (),
                                Err(e) => println!("Error sweeping {}: {}", item.path, e),
                            }
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
