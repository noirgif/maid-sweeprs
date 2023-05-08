use crate::context::MaidContext;
use crate::data;

use async_trait::async_trait;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::PathBuf;

pub(crate) const COLLECTION_NAME: &str = "tags";

#[async_trait]
pub trait Dispatcher<'a, 'b, P: ?Sized>: Send + Sync {
    async fn dispatch(
        &'a self,
        context: &'b dyn MaidContext,
        args: P,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

pub struct Exec {
    pub args: Vec<OsString>,
}

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
impl<'a, 'b> Dispatcher<'a, 'b, PathBuf> for Exec {
    async fn dispatch(
        &'a self,
        _context: &'b dyn MaidContext,
        path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // path replacement
        // TODO: not sure about the arguments
        let mut replaced_args = self.args.iter().map(|arg| match arg.to_str().unwrap() {
            "{}" => ExecArg::RefArg(path.as_os_str()),
            "{/}" => ExecArg::RefArg(path.file_name().unwrap()),
            "{//}" => ExecArg::RefArg(path.parent().unwrap().as_os_str()),
            "{.}" => ExecArg::Arg(path.with_extension("").into_os_string()),
            "{/.}" => ExecArg::RefArg(path.file_stem().unwrap()),
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
impl<'a, 'b> Dispatcher<'a, 'b, (PathBuf, Vec<String>)> for Tag {
    async fn dispatch(
        &'a self,
        context: &'b dyn MaidContext,
        (path, tags): (PathBuf, Vec<String>),
    ) -> Result<(), Box<dyn std::error::Error>> {
        let collection = context.get_db().collection::<data::Item>(COLLECTION_NAME);
        let path = path.to_str().unwrap();
        // TODO: remove copy
        collection
            .insert_one(
                data::Item {
                    path: path.to_string(),
                    tags: tags.to_vec(),
                },
                None,
            )
            .await?;

        Ok(())
    }
}

pub struct File;

#[async_trait]
impl<'a, 'b> Dispatcher<'a, 'b, PathBuf> for File {
    async fn dispatch(
        &'a self,
        context: &'b dyn MaidContext,
        path: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
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

        // TODO: if it has no tag, and not part of a software
        // try to read its name and content
        // if unintelligible, tag it as garbage

        if !tags.is_empty() {
            Tag.dispatch(context, (path, tags)).await?;
        } else if path.is_file() {
            // only tag files as misc
            Tag.dispatch(context, (path, vec![String::from("misc")]))
                .await?;
        }
        Ok(())
    }
}

pub struct Directory;

#[async_trait]
impl<'a, 'b> Dispatcher<'a, 'b, PathBuf> for Directory {
    async fn dispatch(
        &'a self,
        context: &'b dyn MaidContext,
        directory: PathBuf,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let mut entries = tokio::fs::read_dir(directory).await.unwrap();
        while let Some(entry) = entries.next_entry().await.unwrap() {
            let path = entry.path();
            // if it is a file typical of a directory, stop here
            for (file_tags, filename_patterns) in context.get_patterns().typical_files_re.iter() {
                let file_name = path.file_name().unwrap().to_str().unwrap();
                for filename_pattern in filename_patterns {
                    if filename_pattern.is_match(file_name) {
                        Tag.dispatch(
                            context,
                            (
                                path.parent().unwrap().to_owned(),
                                vec![String::from(file_tags)],
                            ),
                        )
                        .await?;
                        return Ok(());
                    }
                }
            }

            // handle special names, and skip tagging
            for (file_tags, filename_pattern) in context.get_patterns().filenames_re.iter() {
                if filename_pattern.is_match(&path.file_name().unwrap().to_str().unwrap()) {
                    Tag.dispatch(context, (path, file_tags.clone())).await?;
                    return Ok(());
                }
            }

            if path.is_dir() {
                match Directory.dispatch(context.clone(), path).await {
                    Ok(_) => {}
                    Err(e) => {
                        println!("Error: {}", e);
                    }
                }
            } else {
                match File.dispatch(context.clone(), path).await {
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
