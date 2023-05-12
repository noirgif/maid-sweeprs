use crate::config;
use crate::context::MaidContext;
use crate::datatype;
use crate::datatype::FileMeta;
use async_trait::async_trait;
use std::error::Error;
use std::ffi::OsStr;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::fs::DirEntry;
use tokio::process::Command;

pub(crate) const COLLECTION_NAME: &str = "tags";

#[derive(Debug)]
pub struct ProcessError {
    pub message: String,
}

impl Display for ProcessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "ProcessError: {}", self.message)
    }
}

impl From<String> for ProcessError {
    fn from(string: String) -> Self {
        ProcessError { message: string }
    }
}

impl From<&str> for ProcessError {
    fn from(string: &str) -> Self {
        ProcessError {
            message: string.to_string(),
        }
    }
}

impl Error for ProcessError {}

#[async_trait]
pub trait Processor<R>: Send + Sync {
    async fn process(self, context: Arc<MaidContext>, args: FileMeta) -> Result<R, ProcessError>;
}

enum FileResult {
    Ok(Vec<String>),
    DirectoryNoTag,
}

unsafe impl Send for FileResult {}
unsafe impl Sync for FileResult {}

pub struct Exec {}

#[async_trait]
impl Processor<()> for Exec {
    async fn process(
        self,
        context: Arc<MaidContext>,
        file_meta: FileMeta,
    ) -> Result<(), ProcessError> {
        let path = file_meta.path;
        let tags = file_meta.tags.unwrap_or_default();

        let exec_args = if let Some(ref args) = context.get_config().exec_args {
            args
        } else {
            return Err("No exec arguments provided".into());
        };

        // To properly and safely replace the arguments, a LR(0) parser is needed.
        // But for now, we just opt for a simple string replace.

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

        let path_with_no_ext: String = path.with_extension("").to_str().unwrap_or_default().into();
        let tags_str = format!("#{}", tags.join("#"));

        if context.is_debug() {
            println!("tags: {:?}", tags);
            println!("path_str: {}", path_str);
            println!(
                "exec_args: {}",
                exec_args
                    .iter()
                    .map(|osstr| osstr.clone().into_string().unwrap_or_default())
                    .collect::<Vec<String>>()
                    .join(" ")
            );
        }

        let replaced_args = exec_args.iter().map(|arg| {
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
            replaced_arg = replaced_arg
                .replace("{0}", &tags_str)
                .replace("{.}", &path_with_no_ext)
                .replace("{/.}", &basename_no_ext)
                .replace("{//}", &dirname)
                .replace("{/}", &basename)
                .replace("{}", &path_str)
                // shell escape and quote
                .replace(r"\", r"\\")
                .replace(r#"""#, r#"\""#);
            // surround with quotes
            format!(r#""{}""#, replaced_arg)
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

        match shell_comm
            .arg(&find_shell.as_ref().unwrap().1)
            .arg(exec_str)
            .spawn()
            .expect("Failed to execute command")
            .wait()
            .await
        {
            Ok(status) => {
                if !status.success() {
                    return Result::Err("Command failed".into());
                }
            }
            Err(e) => {
                return Result::Err(ProcessError::from(format!(
                    "Failed to execute command: {}",
                    e
                )));
            }
        }
        Ok(())
    }
}

pub struct Tag;

#[async_trait]
impl Processor<()> for Tag {
    async fn process(
        self,
        context: Arc<MaidContext>,
        file_meta: FileMeta,
    ) -> Result<(), ProcessError> {
        let collection = context
            .get_db()
            .unwrap()
            .collection::<datatype::FileMetaCompat>(COLLECTION_NAME);
        // TODO: remove copy
        if let Some(tags) = file_meta.tags {
            match collection
                .insert_one(
                    datatype::FileMetaCompat {
                        path: file_meta.path,
                        tags: tags.to_vec(),
                        last_modified: 0,
                    },
                    None,
                )
                .await
            {
                Ok(_) => Ok(()),
                Err(e) => {
                    return Result::Err(ProcessError::from(format!(
                        "Failed to insert file meta: {}",
                        e
                    )));
                }
            }
        } else {
            Result::Err("No tags provided".into())
        }
    }
}

enum Operation {
    Copy(PathBuf),
    Move(PathBuf),
    Remove,
}

struct Move {
    op: Operation,
}

impl Move {
    pub fn new(op: Operation) -> Self {
        Self { op }
    }
}

fn wrap_error<E>(e: E) -> ProcessError
where
    E: Error,
{
    ProcessError::from(format!("IO Error: {}", e.to_string()))
}

fn create_path(path: PathBuf, subdir: &str) -> Result<PathBuf, ProcessError> {
    let target_path = path.join(subdir);
            let result = if !target_path.exists() {
            fs::create_dir_all(&target_path)
        } else {
            Ok(())
        };

        if result.is_err() {
            return Result::Err(format!("Failed to create directory: {}", subdir).into());
        }
        Ok(target_path)
    }

#[async_trait]
impl Processor<()> for Move {
    async fn process(
        self,
        _context: Arc<MaidContext>,
        file_meta: FileMeta,
    ) -> Result<(), ProcessError> {
        // move the file to the directory
        // if the directory does not exist, create it

        let tags = file_meta.tags.as_ref();
        let subdir = if let Some(t) = tags {
            if t.len() > 0 {
                &t[0]
            } else {
                return Result::Err(
                    format!(
                        "No tags available for {}",
                        file_meta.path.to_str().unwrap_or_default()
                    )
                    .into(),
                );
            }
        } else {
            return Result::Err(
                format!(
                    "No tags available for {}",
                    file_meta.path.to_str().unwrap_or_default()
                )
                .into(),
            );
        };


        let exit_result = match (std::env::consts::OS, self.op) {
            ("windows", Operation::Copy(path)) => {
                Command::new("move")
                    .arg(file_meta.path)
                    .arg(create_path(path, subdir)?)
                    .spawn()
                    .map_err(wrap_error)?
                    .wait()
                    .await
            },
            ("windows", Operation::Move(path)) => {
                Command::new("xcopy")
                    .arg(file_meta.path)
                    .arg(create_path(path, subdir)?)
                    .spawn()
                    .map_err(wrap_error)?
                    .wait()
                    .await
            },
            ("windows", Operation::Remove) => {
                Command::new("del")
                    .arg("/f")
                    .arg("/q")
                    .arg(file_meta.path)
                    .spawn()
                    .map_err(wrap_error)?
                    .wait()
                    .await
            },
            (_, Operation::Move(path)) => {
                Command::new("mv")
                    .arg(file_meta.path)
                    .arg(create_path(path, subdir)?)
                    .spawn()
                    .map_err(wrap_error)?
                    .wait()
                    .await
            },
            (_, Operation::Copy(path)) => {
                Command::new("cp")
                    .arg("-r")
                    .arg(file_meta.path)
                    .arg(create_path(path, subdir)?)
                    .spawn()
                    .map_err(wrap_error)?
                    .wait()
                    .await
            },
            (_, Operation::Remove) => {
                Command::new("rm")
                    .arg("-rf")
                    .arg(file_meta.path)
                    .spawn()
                    .map_err(wrap_error)?
                    .wait()
                    .await
            },
        };

        match exit_result {
            Ok(status) => {
                if !status.success() {
                    return Result::Err("Command failed".into());
                } else {
                    Ok(())
                }
            }
            Err(e) => {
                return Result::Err(ProcessError::from(format!(
                    "Failed to execute command: {}",
                    e
                )));
            }
        }
    }
}

pub struct Choice;

#[async_trait]
impl Processor<()> for Choice {
    async fn process(
        self,
        context: Arc<MaidContext>,
        file_meta: FileMeta,
    ) -> Result<(), ProcessError> {
        // skip hidden files
        if file_meta
            .path
            .file_name()
            .map(|s| s.to_string_lossy().starts_with("."))
            .unwrap_or(false)
            && !context.get_config().hidden
        {
            return Ok(());
        }

        // parallelize?
        let mut tasks = vec![];
        if let Some(ref path) = context.get_config().copy_to {
            tasks.push(tokio::task::spawn(
                Move::new(Operation::Copy(path.clone())).process(context, file_meta),
            ));
        } else if context.get_config().save {
            tasks.push(tokio::task::spawn(Tag {}.process(context, file_meta)));
        } else if let Some(ref path) = context.get_config().move_to {
            tasks.push(tokio::task::spawn(
                Move::new(Operation::Move(path.clone())).process(context, file_meta),
            ));
        } else if context.get_config().exec_args.is_some() {
            tasks.push(tokio::task::spawn(Exec {}.process(context, file_meta)));
        } else if context.get_config().delete {
            tasks.push(tokio::task::spawn(
                Move::new(Operation::Remove).process(context, file_meta)
            ));
        }

        if tasks.is_empty() {
            println!("No tasks specified");
            return Ok(());
        }

        for task in tasks {
            match task.await {
                Ok(_) => (),
                Err(e) => {
                    eprintln!("Failed to execute task: {}", e)
                }
            }
        }
        Ok(())
    }
}

pub struct File;

impl File {}

#[async_trait]
impl Processor<FileResult> for File {
    async fn process(
        self,
        context: Arc<MaidContext>,
        file_meta: FileMeta,
    ) -> Result<FileResult, ProcessError> {
        let path = file_meta.path;
        // Match types based on extensions
        let extension = String::from(
            path.extension()
                .and_then(|os_str| os_str.to_str())
                .unwrap_or("")
                .to_ascii_lowercase(),
        );

        // Extension-based tagging
        // Find all that matches
        let mut tags: Vec<String> = context
            .patterns
            .extensions
            .iter()
            .filter_map(|(file_type, extensions)| {
                if extensions.contains(&extension) {
                    Some(file_type.clone())
                } else {
                    None
                }
            })
            .collect();

        if tags.is_empty() && path.is_dir() {
            return Ok(FileResult::DirectoryNoTag);
        }

        // TODO: if a file has no tag, and not part of a software
        // try to read its name and content
        // if unintelligible, tag it as garbage

        if tags.is_empty() {
            tags.push("misc".into());
        }

        // match with given tags and dispatch
        // if no tags specified then dispatch them all
        if file_meta.tags.is_none() || file_meta.tags.as_ref().unwrap().is_empty() {
            // when dispatching, tags means what kind of file it is
            Choice {}
                .process(
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
                        .process(
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
    /// Directly tagging a directory
    async fn handle(self, context: Arc<MaidContext>, file_meta: FileMeta) -> () {
        match (Choice {}.process(context, file_meta).await) {
            Ok(_) => (),
            Err(e) => println!("Error: {}", e),
        }
    }

    /// Calls another dispatcher to process a directory or file
    async fn recurse(self, context: Arc<MaidContext>, entry: DirEntry) -> () {
        let path = entry.path();
        // try to match the folder name with other tags, if it fails, continue
        match File
            .process(
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
                .process(
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

    fn match_special_file(&self, context: &Arc<MaidContext>, path: &Path) -> Option<Vec<String>> {
        context
            .patterns
            .filenames_re
            .iter()
            .find_map(|(file_tags, filename_pattern)| {
                if filename_pattern.is_match(path.file_name().unwrap().to_str().unwrap()) {
                    Some(file_tags.clone())
                } else {
                    None
                }
            })
    }
}

#[async_trait]
impl Processor<()> for Directory {
    async fn process(
        self,
        context: Arc<MaidContext>,
        file_meta: FileMeta,
    ) -> Result<(), ProcessError> {
        if context.is_debug() {
            println!("Processing directory: {:?}", file_meta.path);
        }
        let directory = file_meta.path;
        let mut entries = match tokio::fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(e) => {
                return Err(ProcessError::from(e.to_string()));
            }
        };

        // first pass to filter out typical directories and special files
        let mut filtered_entries = vec![];
        let mut file_tag_tasks: Vec<tokio::task::JoinHandle<_>> = vec![];
        loop {
            // IO error in listing the directory
            let result = entries.next_entry().await;
            if result.is_err() {
                println!("Error: {}", result.err().unwrap());
                break;
            }

            let result = result.unwrap();
            // there is no next entry
            if result.is_none() {
                break;
            }

            let entry = result.unwrap();
            let path = entry.path();

            // if there is a file typical of a kind of directory, tag the directory and stop here
            // typical means there is no ambiguity (but it should be able to have multiple tags)
            // so there is no need to continue
            // TODO: support multiple tags for typical
            let match_result: Option<Vec<String>> =
                context.patterns.typical_files_re.iter().find_map(
                    |(file_tag, filename_patterns)| {
                        if filename_patterns.is_match(path.file_name().unwrap().to_str().unwrap()) {
                            return Some(vec![file_tag.clone()]);
                        }
                        None
                    },
                );

            if let Some(tags) = match_result {
                // if such typical file is found, handle the parent directory
                // and return
                // as this is the only file that matters, we pass up its error
                // with multiple files we ignore them
                return Choice {}
                    .process(
                        context.clone(),
                        FileMeta {
                            path: directory,
                            tags: Some(tags),
                            last_modified: None,
                        },
                    )
                    .await;
            }

            // Find out if it is a special file
            // Special files are not part of a directory, and meaningful even when alone
            // So tagging/moving them sooner or later does not matter

            // if it is a special file, add its handling to the tasks
            if let Some(special_tags) = self.match_special_file(&context, &path) {
                file_tag_tasks.push(tokio::spawn(Self {}.handle(
                    context.clone(),
                    FileMeta {
                        path: path,
                        tags: Some(special_tags),
                        last_modified: None,
                    },
                )));
                // no need to process it again
                // skip to next file
                continue;
            }

            // otherwise proceed to add the files to the list of second pass
            filtered_entries.push(entry);
        }

        file_tag_tasks.extend(
            filtered_entries
                .into_iter()
                .map(|entry| tokio::spawn(Self {}.recurse(context.clone(), entry))),
        );

        for task in file_tag_tasks {
            if let Err(e) = task.await {
                println!("Error: {}", e);
            }
        }

        Ok(())
    }
}
