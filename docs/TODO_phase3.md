# TODO for Phase 3 (Simulation)

## cars

- model cars parking
	- make vanished cars just park again, when possible
	- when parking is full or no parking at goal road, roam until parking is found

- code cleanup
	- figure out responsibility btwn agents and managers, then fix up visibility
	- rng should live in a scenario spec layer, not in the sim itself
	- on a lane vs turn permeates so many places

- better visualization
	- draw moving / blocked colors (gradually more red as they wait longer)
	- draw stop buffer in front/behind of cars

- start implementing a second AORTAish driving model
	- then make cars park/unpark at the correct position

- reversible sim

## bikes

- model bikes as slow cars

## pedestrians

- make them start and end at buildings
	- trim the sidewalk path to the edge of a building
- render overlapping peds reasonably

## General

- savestating a sim has nondet output due to hashes; switching to btree is kind of weird
	- unit test that two savestates of same sim are equal
	- consider overriding encoding for TurnID and such, instead of remembering to stick maps everywhere
- diffing two sim states is tedious no matter what; is there a nice macro-driven deep equals we could do instead?
	- will need programmatic diffs later for pointing out changes to players in A/B tests
- consider refactoring car/ped sim
	- basic structure with actions, react, stepping is same. SimQueue, lookahead, can goto? differs.
