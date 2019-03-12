# Installation

## Dependencies

To build, you need a Linux-like environment with `bash`, `wget`, `unzip`, etc.
You also `osmosis` for the import script.

At runtime if you want to use the screen-capture plugin, you need `scrot`.

## One-time setup

In the future, I'll set up Github binary releases for multiple platforms.

1.   Install Rust, at least 1.31. https://www.rust-lang.org/tools/install

2.   Download the repository: `git clone
     https://github.com/dabreegster/abstreet.git`

3.   Download all input data and build maps. Compilation times will be very
     slow at first. `cd abstreet; ./import.sh`

4.   Optional: Speed up map loading: `./precompute.sh`. Don't be alarmed if
     many maps don't successfully convert.

## Running

There's a bit more to it, but the basics:

`cd editor; cargo run ../data/maps/montlake_no_edits.abst`
