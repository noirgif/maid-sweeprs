use crate::context::MaidContext;
use crate::data;
use crate::data::FileMeta;

use async_trait::async_trait;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::sync::Arc;

pub(crate) const COLLECTION_NAME: &str = "tags";

#[async_trait]
pub trait Dispatcher<'a, 'b, M>: Send + Sync
where
    M: MaidContext + 'a,
    'a: 'b,
{
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        args: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Exec {}

enum ExecArg<'a> {
    RefArg(&'a OsStr),
    Arg(OsString),
}

impl<'a> AsRef<OsStr> for ExecArg<'a> {
    fn as_ref(&self) -> &OsStr {
        match self {
            ExecArg::RefArg(arg) => arg.clone(),
            ExecArg::Arg(arg) => arg.as_os_str(),
        }
    }
}

#[async_trait]
impl<'a, 'b, M> Dispatcher<'a, 'b, M> for Exec
where
    M: MaidContext + 'a,
    'a: 'b,
{
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = file_meta.path;
        // path replacement
        // TODO: not sure about the arguments
        let tags = file_meta.tags.unwrap_or_default();

        let exec_args: Vec<OsString>;
        if let Some(args) = context.get_exec_args() {
            exec_args = args;
        } else {
            return Result::Err("No exec arguments provided".into());
        }

        let mut replaced_args = exec_args.iter().map(|arg| match arg.to_str().unwrap() {
            "{}" => ExecArg::RefArg(path.as_os_str()),
            "{/}" => ExecArg::RefArg(path.file_name().unwrap()),
            "{//}" => ExecArg::RefArg(path.parent().unwrap().as_os_str()),
            "{.}" => ExecArg::Arg(path.with_extension("").into_os_string()),
            "{/.}" => ExecArg::RefArg(path.file_stem().unwrap()),
            "{1}" => ExecArg::Arg(
                tags.get(0)
                    .and_then(|s| Some(OsString::from(s)))
                    .unwrap_or(OsString::new()),
            ),
            _ => ExecArg::RefArg(arg),
        });

        // TODO: pass multithreading context
        if let Some(exec_path) = replaced_args.next() {
            tokio::process::Command::new(exec_path)
                .args(replaced_args)
                .spawn()
                .expect("Failed to execute command")
                .wait()
                .await?;
            Ok(())
        } else {
            Result::Err("No exec arguments provided".into())
        }
    }
}

pub struct Tag;

#[async_trait]
impl<'a, 'b, M> Dispatcher<'a, 'b, M> for Tag
where
    M: MaidContext + 'a,
    'a: 'b,
{
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let collection = context
            .get_db()
            .collection::<data::FileMetaCompat>(COLLECTION_NAME);
        // TODO: remove copy
        if let Some(tags) = file_meta.tags {
            collection
                .insert_one(
                    data::FileMetaCompat {
                        path: file_meta.path,
                        tags: tags.to_vec(),
                        last_modified: 0,
                    },
                    None,
                )
                .await?;
            Ok(())
        } else {
            Result::Err("No tags provided".into())
        }
    }
}

pub struct File;

#[async_trait]
impl<'b, 'a: 'b, M: crate::context::MaidContext + 'a> Dispatcher<'a, 'b, M> for File {
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let path = file_meta.path;
        // Match types based on extensions
        let extension = String::from(
            path.extension()
                .and_then(|os_str| os_str.to_str())
                .unwrap_or(""),
        );

        // Extension-based tagging
        let mut tags = vec![];
        for (file_type, extensions) in context.get_patterns().extensions.iter() {
            if extensions.contains(&extension) {
                tags.push(file_type.clone());
            }
        }

        // Special cases
        for (file_tags, filename_pattern) in context.get_patterns().filenames_re.iter() {
            if filename_pattern.is_match(path.file_name().unwrap().to_str().unwrap()) {
                tags.append(&mut file_tags.clone());
            }
        }

        // TODO: more file name/date based tagging
        if tags.is_empty() {
            // TODO: if it has no tag, and not part of a software
            // try to read its name and content
            // if unintelligible, tag it as garbage
            tags = vec![String::from("misc")];
        }

        Tag.dispatch(
            context,
            FileMeta {
                path: path,
                tags: Some(tags),
                last_modified: None,
            },
        )
        .await?;

        Ok(())
    }
}

pub struct Directory;

#[async_trait]
impl<'a, 'b: 'a, M: MaidContext + 'b> Dispatcher<'b, 'a, M> for Directory
where
    M: MaidContext,
{
    async fn dispatch(
        &'a self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = file_meta.path;

        let mut entries = tokio::fs::read_dir(directory).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let path = entry.path();
            // if it is a file typical of a directory, stop here
            for (file_tags, filename_patterns) in context.get_patterns().typical_files_re.iter() {
                let file_name = path.file_name().unwrap().to_str().unwrap();
                for filename_pattern in filename_patterns {
                    if filename_pattern.is_match(file_name) {
                        Tag.dispatch(
                            context.clone(),
                            FileMeta {
                                path: path.parent().unwrap().to_owned(),
                                tags: Some(vec![String::from(file_tags)]),
                                last_modified: None,
                            },
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }

            // handle special names, and skip tagging
            for (file_tags, filename_pattern) in context.get_patterns().filenames_re.iter() {
                if filename_pattern.is_match(&path.file_name().unwrap().to_str().unwrap()) {
                    Tag.dispatch(
                        context.clone(),
                        FileMeta {
                            path,
                            tags: Some(file_tags.clone()),
                            last_modified: None,
                        },
                    )
                    .await?;
                    return Ok(());
                }
            }

            if path.is_dir() {
                match Directory
                    .dispatch(
                        context.clone(),
                        FileMeta {
                            path: path,
                            tags: None,
                            last_modified: None,
                        },
                    )
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            } else {
                match File
                    .dispatch(
                        context.clone(),
                        FileMeta {
                            path: path,
                            tags: None,
                            last_modified: None,
                        },
                    )
                    .await
                {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            }
        }

        Ok(())
    }
}
