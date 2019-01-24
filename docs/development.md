# Development notes

Find packages to upgrade: `cargo outdated -R`

Diff screencaps: http://www.imagemagick.org/Usage/compare/#methods

Cross-compilation: https://github.com/rust-embedded/cross

Debug OpenGL calls:
	apitrace trace --api gl ../target/debug/editor ../data/raw_maps/montlake.abst
	qapitrace editor.trace
	apitrace dump editor.trace
