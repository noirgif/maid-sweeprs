# Maid Sweeper

If you have a lot of files unorganized, and do not want to break up directories like code projects and applications, this <s>tool</s> maid is for you.

This is the Rust version of [maid-sweeper](https://github.com/noirgif/maid-sweeper), a <s>tool</s> service to classify files and directories.

If desired, the maid can practice Danshari given permission. For example, she can [sell your unused iPad for money](https://comic-days.com/episode/3269754496647364302).

Like Toki, she has two modes:

`tag`: Label the files/directories automatically, based on their types and names.

- code projects and application directories are labeled, and their children are not scanned
  - if there is a DLL, you know what it is for, the maid also knows.
- others are labeled based on the extensions, or names if its name indicates that it is a special kind of file.

`sweep`: Carry out actions based on the labels.

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


### With MongoDB
1. Call `maid tag -d --mongodb-host <MONGODB_URL> ~/Videos/Study`, then you can find tagged entries in the database. Sweeping works on all directories tagged.
2. Call `maid sweep -d --mongodb-host <MONGODB_URL> -x rm -rf {}`, and the maid is going to remove all 'video' or 'game' tagged files and directories.
    * Any other commands is OK as well
    * {1}, {2}, {3} is the first, second and third tag, whereas {0} is all the tags, concatenated like #video#game#, like in TagLyst, for you to append the basename after.
        * Haven't implemented multiple tags, so {1} to go

### Without MongoDB

Call `maid sweep ~/Videos/Study -x mkdir -p Tagged/{1} "&&" mv {} {1}`, which moves all tagged files and directories to `Tagged` directory, categorized.
I didn't know that before, but things like `&&` need to be escaped.

## Ideas

- Multithreading
- Tags based on time
    * How does it affect other tags? If not why bother?
    * Maybe not tag, but just metadata
    * There will be IO cost
- Group similarly named files: 01.jpg, 02.jpg, etc.
- Understand human language so they can toss away garbage
- Optionally clean up the database after sweeping.