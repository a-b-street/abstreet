# TODO - Refactoring

- easier way to define magic tuneable constants
	- and maybe to recalculate fixedish things if they change?

## Map layer

- fixed precision math
	- more careful geom types, with negative/positive cases
	- also bounds?
	- cant get rid of the ccw intersection check... different answer in some cases that looks bad

- maybe also the time to split into different lane types? what's similar/not between them?
	- graph querying?
	- rendering (and other UI/editor interactions)?
	- sim state?
	- Sidewalk, Parking, Street

- make synthetic use raw stuff directly?
	- lonlat vs pt is annoying; have to use bounds to balloon to world at least once

## Sim layer

- consider refactoring car/ped sim
	- basic structure with actions, react, stepping is same. SimQueue, lookahead, can goto? differs.

- detangle sim managers... but first, figure out how to capture stacktraces
	- manual call at a fxn to dump its stacktrace somewhere (to a file? ideally shared global state to dedupe stuff)
	- macro to insert a call at the beginning of a fxn
	- macro to apply a macro to all fxns in an impl
	- then i can manually edit a few places when I want to gather data
	- https://en.wikipedia.org/wiki/File:A_Call_Graph_generated_by_pycallgraph.png
- figure out responsibility btwn agents and managers, then fix up visibility
- things like ParkingSimState have so many methods -- some are only
  meant for spawner, or driving/walking to query. separate out some
  traits.

- on a lane vs turn permeates so many places
