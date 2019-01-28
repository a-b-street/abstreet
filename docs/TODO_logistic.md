# TODO - Project logistics

- enable more clippy lints
- enforce consistent style (derive order, struct initialization order)
- cross-platform builds

- document map model
	- diagram of data sources and stages
	- explanation of intermediate formats
	- autogenerate diagrams of the data schemas
	- list invariants

- update with mission statement (democratized urb p, that quote, refashion existing space cheaply)
- trailer
	- show common parts of routes in A/B, point of divergence
	- "Two parallel universes sit at your fingertips, and with the flick of a key, you can glide between the two. Buses jumping past traffic in one world, snarly traffic jam in the other. An A/B test revealing what currently is, and what could be, compared meticulously and deterministically. A/B Street -- which world do you prefer?"

## Stability

- test results per git commit
	- way to view later
	- also could be benchmarks; just arbitrary data over time
	- also screenshots

- things fixed-pt should solve
	- determinism tests failing
	- polyline intersection() finding a line hit, but then failing on get_slice_ending_at

- improve test code and explore problems
	- big timestep, does follow error blow up?
	- alternative to scenario is a sequence of commands for tests
		- spawning code is becoming a BIG mess
	- more tests: bikes, cars, peds starting/ending at borders

- layered invariants
	- first: all the maps fully convert and display in some form; all tests pass or are disabled
	- eventually: every intersection has at least a turn, minimum lengths enforced, etc

### Current major geometry problems

- line intersection code is giving completely silly results for 23rd
	- Line::new(
  Pt2D::new(2220.510790392476, 17.372151672558502),
  Pt2D::new(2220.5166115696447, 15.152159304577435),
) and Line::new(
  Pt2D::new(2220.5127307848657, 16.63215421656464),
  Pt2D::new(2220.514671177255, 15.892156760571009),
) intersect, but first line doesn't contain_pt(Pt2D(2220.495346552271, 22.98459165963943))

- bad shifted polylines on 45th st in 23rd map
