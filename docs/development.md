# Development notes

Find packages to upgrade: `cargo outdated -R`

Diff screencaps: http://www.imagemagick.org/Usage/compare/#methods

Cross-compilation: https://github.com/rust-embedded/cross

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
