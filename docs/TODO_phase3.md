# TODO for Phase 3 (Simulation)

## cars

- code cleanup
	- figure out responsibility btwn agents and managers, then fix up visibility
	- on a lane vs turn permeates so many places

- better visualization
	- draw moving / blocked colors (gradually more red as they wait longer)
	- make lookahead buffer follow the shape of the road and extend into other lanes and stuff

- reversible sim

## bikes

- model bikes as slow cars

## pedestrians

- render overlapping peds reasonably

## General

- savestating a sim has nondet output due to hashes; switching to btree is kind of weird
	- consider overriding encoding for TurnID and such, instead of remembering to stick maps everywhere
- diffing two sim states is tedious no matter what; is there a nice macro-driven deep equals we could do instead?
	- will need programmatic diffs later for pointing out changes to players in A/B tests
- consider refactoring car/ped sim
	- basic structure with actions, react, stepping is same. SimQueue, lookahead, can goto? differs.
