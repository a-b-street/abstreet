# Data organization

A/B Street includes lots of large binary files to represent converted maps,
scenarios, and prebaked simulation results. The files are too large to store in
git, but the files are still logically tied to a version of the code, since the
format sometimes changes. Additionally, all of the files are too large to
include in the .zip release that most people use, but it should still be
possible for players to download the optional content. Also, there are different
versions of the game floating around, on native and web, that have to be
associated with the proper version of these files.

It's all slightly confusing, so this page describes how it all works.

## The data itself

If you peek into the `data/` directory, it's mainly split into 3 subdirectories.
`system/` is used when running the game and is the subject of this page.
`input/` is used to store input and intermediate files for importing maps, and
only developers running the importer should care about it. `player/` contains
local settings, map edits, and other data created in-game.

`data/MANIFEST.json` is a listing of all files in `data/system/`, along with
their size and md5sum. Different tools compare this manifest to the local
filesystem to figure out what to do.

There are also some other scripts and files in `data/`, but they should probably
be moved.

## Where the data is stored

`data/system/` and `data/input/` are stored in Amazon S3, at
http://abstreet.s3-website.us-east-2.amazonaws.com. This S3 bucket is organized
into versions: `dev`, `0.2.17`, `0.2.18`, etc. `dev` represents the latest
version of all data files. The numbered versions correspond to
[releases](https://github.com/dabreegster/abstreet/releases) and only contain
`data/system/`, not `data/input/`. Depending how large these directories grow
over time, I'll commit to keeping around at least 3 of the previous numbered
versions, but I might delete older ones after that.

In lieu of a proper document for the release process, the commands used to make
a versioned copy of the data are something like:

```
aws s3 cp --recursive s3://abstreet/dev/data/system s3://abstreet/0.2.17/data/system
```

## Native, running from source

For people building the game [from source](index.md), the process to keep data
files fresh is to `cargo run --bin updater`. This tool calculates md5sums of all
local files, then compares it with the checked-in `data/MANIFEST.json`. Any
difference results in a local file being deleted or a new file from S3 being
downloaded. By editing `data/player/data.json` manually or using the UI in the
game (found by loading a map, then choosing to download more maps), somebody can
opt into downloading "extra/optional" cities.

## Native, running from a release .zip

When the weekly .zip binary release for Mac, Linux, and Windows is produced, the
`game` crate is built with `--features release_s3`. When the downloader UI is
opened in-game, this causes downloads to occur from a versioned S3 directory,
like `0.2.17`, depending on the version string compiled into the game at that
time. So somebody can run off the weekly release, opt into more cities, and get
the correct version of the files, even if the format has changed in `/dev/`
since then.

## Web, running locally

The strategy for managing files gets more interested when the game is compiled
to WebAssembly, since browsers can't read from the local filesystem.
`game/src/load.rs` contains some crazy tricks to instead make asynchronous HTTP
requests through the browser. When using `game/run_web.sh`, the files are served
through a local HTTP server and symlinked to the local copy of `data/system/`.

Not all files are loaded through HTTP; some are actually statically compiled
into the `.wasm` file itself! `abstutil/src/io_web.rs` does this magic using the
`include_dir` crate. Only a few critical large files, needed at startup, are
included. There's an IO layer for listing and reading files that, on web, merges
results from the bundled-in files and the remote files that're declared to exist
in the bundled-in copy of `data/MANIFEST.json`.

## Web, from S3

Everything's the same, except building with `--features wasm_s3` causes the game
to make HTTP requests to the S3 bucket, instead of localhost. The web version
always pins to `/dev`, never a release version of the data, since the web client
is always updated along with the data, for now.
