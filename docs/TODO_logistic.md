# TODO - Project logistics

- enable more clippy lints
- enforce consistent style (derive order, struct initialization order)

- trailer
	- show common parts of routes in A/B, point of divergence
	- "Two parallel universes sit at your fingertips, and with the flick of a key, you can glide between the two. Buses jumping past traffic in one world, snarly traffic jam in the other. An A/B test revealing what currently is, and what could be, compared meticulously and deterministically. A/B Street -- which world do you prefer?"

## Tooling

- play with https://github.com/glennw/thread_profiler
- and https://github.com/ferrous-systems/cargo-flamegraph
- display percentage breakdowns in Timer (need tree structure)

## Stability

- test results per git commit
	- https://github.com/spotify/git-test
	- way to view later
	- also could be benchmarks; just arbitrary data over time

- layered invariants
	- first: all the maps fully convert and display in some form; all tests pass or are disabled
	- slowly hone away at problems currently with errors printed (like bad pl shift angles)
	- eventually: every intersection has at least a turn, minimum lengths enforced, etc

- useful unit tests
	- for a given intersection with lanes, check all the turns generated
