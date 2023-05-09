use crate::config;
use crate::context::MaidContext;
use crate::data;
use crate::data::FileMeta;
use async_trait::async_trait;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::path::Path;
use std::sync::Arc;
use tokio::fs::DirEntry;

pub(crate) const COLLECTION_NAME: &str = "tags";

#[async_trait]
pub trait Dispatcher<'a, 'b, M, R>: Send + Sync
where
    M: MaidContext + 'a,
    'a: 'b,
{
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        args: FileMeta,
    ) -> Result<R, Box<dyn std::error::Error>>;
}

enum FileResult {
    Ok(Vec<String>),
    DirectoryNoTag,
}

unsafe impl Send for FileResult {}
unsafe impl Sync for FileResult {}

pub struct Exec {}

#[async_trait]
impl<'a, 'b, M> Dispatcher<'a, 'b, M, ()> for Exec
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
        let tags = file_meta.tags.unwrap_or_default();

        let exec_args: Vec<OsString>;
        if let Some(args) = context.get_exec_args() {
            exec_args = args;
        } else {
            return Result::Err("No exec arguments provided".into());
        }

        let path_str = String::from(path.to_str().unwrap_or(""));
        let basename: String = path
            .file_name()
            .and_then(OsStr::to_str)
            .unwrap_or("")
            .into();
        let dirname: String = path
            .parent()
            .and_then(Path::to_str)
            .unwrap_or_default()
            .into();
        let basename_no_ext: String = path
            .file_stem()
            .and_then(OsStr::to_str)
            .unwrap_or_default()
            .into();

        // UNDONE: properly parse the command

        let path_with_no_ext: String = path.with_extension("").to_str().unwrap_or_default().into();
        let tags_str = format!("#{}", tags.join("#"));
        if context.is_debug() {
            println!("tags: {:?}", tags);
            println!("path_str: {}", path_str);
        }

        let replaced_args = exec_args.iter().map(|arg| {
            if context.is_debug() {
                println!("arg: {}", arg.to_str().unwrap_or(""));
            }
            // do not replace shell special strings
            match arg.to_str().unwrap_or("") {
                "|" | "&" | "&&" | "<" | ">" | ">>" | "<<" => {
                    return arg.clone().to_str().unwrap_or("").to_owned();
                }
                _ => (),
            }

            let mut replaced_arg = arg.clone().into_string().unwrap();
            if tags.len() > 0 {
                replaced_arg = replaced_arg.replace("{1}", &tags[0]);
            }
            if tags.len() > 1 {
                replaced_arg = replaced_arg.replace("{2}", &tags[1]);
            }
            if tags.len() > 2 {
                replaced_arg = replaced_arg.replace("{3}", &tags[2]);
            }
            replaced_arg = replaced_arg.replace("{0}", &tags_str);
            replaced_arg = replaced_arg.replace("{.}", &path_with_no_ext);
            replaced_arg = replaced_arg.replace("{/.}", &basename_no_ext);
            replaced_arg = replaced_arg.replace("{//}", &dirname);
            replaced_arg = replaced_arg.replace("{/}", &basename);
            replaced_arg = replaced_arg.replace("{}", &path_str);
            // shell escape and quote
            replaced_arg = replaced_arg.replace(r"\", r"\\");
            replaced_arg = replaced_arg.replace(r#"""#, r#"\""#);
            replaced_arg = format!(r#""{}""#, replaced_arg);

            replaced_arg
        });

        // TODO: pass multithreading context
        let find_shell = config::find_shell();
        let mut shell_comm = match find_shell {
            Some((ref shell, ref _arg1)) => tokio::process::Command::new(shell),
            _ => return Result::Err("No shell found!".into()),
        };

        let exec_str = replaced_args.into_iter().collect::<Vec<String>>().join(" ");

        if context.is_debug() {
            println!("exec_str: {:?} {}", find_shell.as_ref().unwrap(), exec_str);
        }

        shell_comm
            .arg(&find_shell.as_ref().unwrap().1)
            .arg(exec_str)
            .spawn()
            .expect("Failed to execute command")
            .wait()
            .await?;
        Ok(())
    }
}

pub struct Tag;

#[async_trait]
impl<'a, 'b, M> Dispatcher<'a, 'b, M, ()> for Tag
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

pub struct Choice;

#[async_trait]
impl<'b, 'a: 'b, M: MaidContext + 'a> Dispatcher<'a, 'b, M, ()> for Choice {
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        if context.get_exec_args().is_some() {
            Exec {}.dispatch(context, file_meta).await?;
        } else {
            Tag {}.dispatch(context, file_meta).await?;
        }
        Ok(())
    }
}

pub struct File;

#[async_trait]
impl<'b, 'a: 'b, M: crate::context::MaidContext + 'a> Dispatcher<'a, 'b, M, FileResult> for File {
    async fn dispatch(
        &'b self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<FileResult, Box<dyn std::error::Error>> {
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

        // TODO: more file name/date based tagging
        if tags.is_empty() {
            // TODO: if it has no tag, and not part of a software
            // try to read its name and content
            // if unintelligible, tag it as garbage
            if path.is_dir() {
                return Ok(FileResult::DirectoryNoTag);
            } else {
                tags = vec![String::from("misc")];
            }
        }

        // match with given tags and dispatch
        // if no tags specified then dispatch them all
        if file_meta.tags.is_none() || file_meta.tags.as_ref().unwrap().is_empty() {
            Choice {}
                .dispatch(
                    context,
                    FileMeta {
                        path: path,
                        tags: Some(tags),
                        last_modified: None,
                    },
                )
                .await?;
            return Ok(FileResult::Ok(vec![]));
        }

        for tag in tags.iter() {
            if let Some(ref filter_tags) = file_meta.tags {
                if filter_tags.contains(tag) {
                    Choice {}
                        .dispatch(
                            context,
                            FileMeta {
                                path: path,
                                tags: Some(tags),
                                last_modified: None,
                            },
                        )
                        .await?;
                }
            }
            break;
        }
        Ok(FileResult::Ok(vec![]))
    }
}

pub struct Directory;

impl Directory {
    async fn classify_and_tag<'a, 'b, M>(&'b self, context: Arc<M>, entry: DirEntry) -> ()
    where
        M: MaidContext + 'a,
        'a: 'b,
    {
        let path = entry.path();
        // try to match the folder name with other tags, if it fails, continue
        match File
            .dispatch(
                context.clone(),
                FileMeta {
                    path: entry.path().clone(),
                    tags: None,
                    last_modified: None,
                },
            )
            .await
        {
            Ok(FileResult::DirectoryNoTag) => (),
            Ok(FileResult::Ok(_)) => return,
            Err(e) => println!("Error: {}", e),
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
                Ok(_) => (),
                Err(e) => println!("Error: {}", e),
            }
        }
    }
}

#[async_trait]
impl<'a, 'b: 'a, M: MaidContext + 'b> Dispatcher<'b, 'a, M, ()> for Directory
where
    M: MaidContext,
{
    async fn dispatch(
        &'a self,
        context: Arc<M>,
        file_meta: FileMeta,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let directory = file_meta.path;

        let mut entries = tokio::fs::read_dir(&directory).await.unwrap();
        let mut filtered_entries = vec![];
        loop {
            let result = entries.next_entry().await;
            match result {
                Ok(Some(entry)) => {
                    let path = entry.path();
                    // if there is a file typical of a kind of directory, tag the directory and stop here
                    let mut match_result: Option<Vec<String>> = None;
                    for (file_tag, filename_patterns) in
                        context.get_patterns().typical_files_re.iter()
                    {
                        if match_result.is_some() {
                            break;
                        }

                        for filename_pattern in filename_patterns {
                            if filename_pattern
                                .is_match(path.file_name().unwrap().to_str().unwrap())
                            {
                                match_result = Some(vec![file_tag.clone()]);
                                break;
                            }
                        }
                    }
                    if let Some(tags) = match_result {
                        // if such typical file is found, handle the parent directory
                        // and return
                        return Choice {}
                            .dispatch(
                                context.clone(),
                                FileMeta {
                                    path: directory,
                                    tags: Some(tags),
                                    last_modified: None,
                                },
                            )
                            .await;
                    }

                    // If it is not found, find out if it is a special file
                    // Special files are not part of a directory, and meaningful even when alone
                    // So tagging/moving them sooner or later does not matter
                    let mut special_tags = vec![];
                    for (file_tags, filename_pattern) in context.get_patterns().filenames_re.iter()
                    {
                        if filename_pattern.is_match(path.file_name().unwrap().to_str().unwrap()) {
                            for tag in file_tags.iter() {
                                special_tags.push(tag.clone());
                            }
                        }
                    }
                    if !special_tags.is_empty() {
                        match (Choice {}
                            .dispatch(
                                context.clone(),
                                FileMeta {
                                    path: path.clone(),
                                    tags: Some(special_tags),
                                    last_modified: None,
                                },
                            )
                            .await)
                        {
                            Ok(_) => (),
                            Err(e) => println!("Error: {}", e),
                        }
                        continue;
                    }

                    // otherwise proceed to classify the file
                    filtered_entries.push(entry);
                }
                Err(e) => {
                    println!("Error: {}", e);
                    break;
                }
                _ => break,
            }
        }
        let tasks = filtered_entries.into_iter().map(|entry| tokio::spawn(
            Self {}.classify_and_tag(context.clone(), entry),
        ));
        for task in tasks {
            task.await?;
        }

        Ok(())
    }
}
