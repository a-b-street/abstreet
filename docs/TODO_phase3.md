# TODO for Phase 3 (Simulation)

## cars

- code cleanup
	- figure out responsibility btwn agents and managers, then fix up visibility
	- things like ParkingSimState have so many methods -- some are only
	  meant for spawner, or driving/walking to query. separate out some
          traits.
	- on a lane vs turn permeates so many places

- better visualization
	- draw moving / blocked colors (gradually more red as they wait longer)
	- make lookahead buffer follow the shape of the road and extend into other lanes and stuff

- reversible sim

- be careful
	- could see if we ever have a lookahead constraint to deaccel more than
	  what we're capable of. it might mask problems. but since things like
          accel_to_stop_in_dist don't have a careful notion of how much time will pass,
          they recommend big rates sometimes.
	- no way for an agent to request a turn and ASAP have it granted. are there cases where they might slow down unnecessarily?

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
