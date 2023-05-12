# Maid Sweeper

If you have a lot of files unorganized, and do not want to break up directories like code projects and applications, this <s>tool</s> maid is for you.

This is the Rust version of [maid-sweeper](https://github.com/noirgif/maid-sweeper), a <s>tool</s> service to classify files and directories.

If desired, the maid can practice Danshari given permission. For example, she can [sell your unused iPad for money](https://comic-days.com/episode/3269754496647364302).

Like Toki in Blue Archive, she is a maid with two modes:

Online: Label the files/directories and save them in a mongodb database. When dispatching those files it can also read the entries from the database. Useful if you want to sweep the same directory multiple times or keep a statistics of the files.

Offline: Label the files/directories and dispatch them immediately. Useful if you want to sweep a directory once.

- code projects and application directories are labeled, and their children are not scanned
  - if there is a DLL, you know what it is for, the maid also knows.
- others are labeled based on the extensions, or names if its name indicates that it is a special kind of file.


## Feature

* Uses Tokio for asynchronous processing. <s>Toki, uohhhhh😭😭😭</s>
* MongoDB for fast indexing if you want to 
* Save time by not scanning every single file inside code and program directories and not checking the metadata
* Use yaml to configure the rules and tags
* Kyoufu!

## Installation

1. Run `cargo install maid-sweeprs`.
2. Copy `maidsweep.yaml` to `~/.maidsweep.yaml`. Or any place you like, in that case you need to specify the path with `-c` option.
  * Feel free to modify the rules
3. Install MongoDB and start the service (optional).

## Usage

`maid [--use-mongodb] [--mongodb-host <MONGODB_URL>] [-c <CONFIG_PATH>] [-t <TAGS>] PATH ACTIONS`

* `--use-mongodb` uses mongodb entries for sweeping.
* `--mongodb-host` specifies the mongodb url, default is `mongodb://localhost:27017`.
* `-c` specifies the path to the config file, default is `~/.maidsweep.yaml`.
* `-t` specifies files with which tags to sweep, default is any tag.

`ACTIONS = [-x ARGS] | [--cp <DESTINATION>] | [--mv <DESTINATION>] | [--save]`

* `-x` is like `--exec` in find, and `-x` in `fd`, it executes a command.
* `--cp`, `--mv` copies or moves a file to `<destination>/<first tag of the file>/`.
* `--save` saves the entries to the database, you can then specify `--use-mongodb` to read the entries from the database for sweeping.


### With MongoDB
1. Start a MongoDB service.
2. Call `maid --mongodb-host <MONGODB_URL> ~/Videos/Study --save`, then you can find tagged entries in the database. Sweeping works on all directories tagged.
3. Call `maid --use-mongodb --mongodb-host <MONGODB_URL> -t video game --mv classified`, and the maid is going to move all 'video' or 'game' tagged files and directories to a `classified/video`, and `classified.

### Without MongoDB

Call `maid ~/Videos/Study -x --cp Tagged`, the maid copies all tagged files and directories to `Tagged` directory, categorized.



## Ideas

- Multithreading
- Tags based on time
    * How does it affect other tags? If not why bother?
    * Maybe not tag, but just metadata
    * There will be IO cost
- Group similarly named files: 01.jpg, 02.jpg, etc.
- Understand human language so they can toss away garbage
- Optionally clean up the database after sweeping.