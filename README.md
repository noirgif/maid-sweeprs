# Maid Sweeper

This is the Rust version of [maid-sweeper](github.com/noirgif/maid-sweeper), a tool to clean up your files and directories.

No, instead of cleaning the unused files, it calls a maid to label them and sweep them under the rug.

If desired, the maid can practice Danshari given permission. For example, she can [sell your unused iPad for money](https://comic-days.com/episode/3269754496647364302).

Like Toki, she has two modes:

`tag`: Label the files/directories automatically, based on their types and names.

- code projects and application directories are labeled, and their children are not scanned
- others are labeled based on the extensions

`sweep`: Carry out actions based on the labels.

## Feature

* Toki, ohhhhh
* MongoDB for fast indexing
* Save time by not scanning every single file inside code and program directories and not checking the metadata
* Use yaml to configure the rules and tags
* Kyoufu!

## Installation

1. Run `cargo install maid-sweeprs`.
2. Install MongoDB and start the service (optional).

## Usage

Copy `maidsweep.yaml` to `~/.maidsweep.yaml`. Or any place you like, but you want to specify the path with `-c` option.

### Tagging



### With MongoDB
1. Call `maid tag -d --mongodb-host <MONGODB_URL> ~/Videos/Study`, then you can find tagged entries in the database. Sweeping works on all directories tagged.
2. Call `maid sweep -d --mongodb-host <MONGODB_URL> -x rm -rf {}`, and the maid is going to remove all 'video' or 'game' tagged files and directories.
    * Any other commands is OK as well
    * {1}, {2}, {3} is the first, second and third tag, whereas {0} is all the tags, concatenated like #video#game#, like in TagLyst, for you to append the basename after.
        * Haven't implemented multiple tags, so {1} to go

### Without MongoDB

Call `maid tag ~/Videos/Study -x mkdir -p Tagged/{1} "&&" mv {} {1}`, which moves all tagged files and directories to `Tagged` directory.
I didn't know that, but things like `&&` need to be escaped.

## Ideas

- Multithreading
- Tags based on time
    * How does it affect other tags? If not why bother?
    * Maybe not tag, but just metadata
    * There will be IO cost
- Group similarly named files: 01.jpg, 02.jpg, etc.
- Understand human language so they can toss away garbage
- Optionally clean up the database after sweeping.
- Single line mode: do the tag, sweep, and clean up database entries with a single command.
    * Skip MongoDB if possible
- Generate and reads from config files