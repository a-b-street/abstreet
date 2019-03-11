# Development notes

Find packages to upgrade: `cargo outdated -R`

Diff screencaps: http://www.imagemagick.org/Usage/compare/#methods

Debug OpenGL calls:
	apitrace trace --api gl ../target/debug/editor ../data/raw_maps/montlake.abst
	qapitrace editor.trace
	apitrace dump editor.trace

## Profiling

apt-get install google-perftools libgoogle-perftools-dev

Follow Usage from https://crates.io/crates/cpuprofiler

Run editor or headless with --enable_profiler
google-pprof --no_strip_temp ../target/debug/editor profile
google-pprof --no_strip_temp ../target/release/headless profile
top30 --cum

## Building releases

Cross-compilation notes: https://github.com/rust-embedded/cross

cross build --release --target x86_64-pc-windows-gnu --bin editor
wine target/x86_64-pc-windows-gnu/release/editor.exe data/maps/montlake_no_edits.abst

Problems:
- build.rs tries to invoke python
	- port the python script to rust ;)
- GLSL 1.40 not supported (in wine at least)
