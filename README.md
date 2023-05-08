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

## Usage

1. Copy `patterns.yaml` to `~/.maid-sweepers/patterns.yaml`.
2. Start a MongoDB instance.
3. Call `maid tag D:\Study`, then you can find tagged entries in the database. Sweeping works on all directories tagged.
4. Call `maid sweep -t video game -x del \q \f {}`, and the maid is going to remove all 'video' or 'game' tagged files and directories.
    * Any other commands is OK as well

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