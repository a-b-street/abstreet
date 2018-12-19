# Code organization-related design notes

## Associated data / ECS

So far, the different structures for representing everything associated with a
road/intersection/etc strongly resembles ECS. Would explicitly using an ECS
library help?

http://www.gameprogrammingpatterns.com/component.html

Road has different representations:

- protobuf
- runtime map_model (mostly focusing on the graph)
- UI wrapper + geometry for simulation (should probably tease this apart)
- "control" layer for editable policies
- Queue of cars on the road

It could be useful to bundle together a context-like object of Map, GeomMap,
ControlMap, DrawMap, etc.

Need to figure out how to handle reaping old IDs for transient objects like
cars, but also things like modified roads. Slot maps?

Sort of related -- http://smallcultfollowing.com/babysteps/blog/2018/11/01/after-nll-interprocedural-conflicts/

## Everything as FSMs

Driving and walking layer are both kind of broken, since they know about
parking spots and bus stops. Feels like they need to be dumb, mechanical layers
that're guided by higher-level behaviors, which understand trips and such.

Could be simpler to flatten. Call each sim and dont affect other sims, then
process all events last to do transitions. except maybe one sim expects the
transition to happen immediately, so have to process events between each one?

Just to kind of document the dependency/call-chain right now...

- sim step
	- TODO spawner step
	- driving step
		- router
			- transit.get_action_when_stopped_at_end
				- this changes bus state (wouldnt it be nicer to 
		- foreach parked car as a result, add to parking sim and tell spawner that car reached spot


ask: a way to statically or at runtime print the call-chain, stopping at std libs?
	- whenever we push onto events, dump stack?

for each api method in each sim, manually look at its call chain

- transit
	- create_empty_route, get_route_starts, bus_created
		- spawn.seed_bus_route
			- sim.seed_bus_route
	- get_action_when_stopped_at_end (changes bus state, returns new path), get_dist_to_stop_at
		- router.react_before_lookahead
			- driving per car react
				- driving step
	- step (lots of state transitions happen here, but call stack is simple!)
- walking
	- ped_joined_bus
		- transit.step

A good measue of how well-structured the code is: how hard would it be to add a
few more delays / states, like time after parking the car (out of the way of
the driving sim) but before getting out of the car?

### Notes on capturing this

https://www.reddit.com/r/rust/comments/9d8gse/higher_level_api_than_syn_for_static_analysis/

- figure out how to capture stacktraces kinda optionally
	- manual call at a fxn to dump its stacktrace somewhere (to a file? ideally shared global state to dedupe stuff)
	- macro to insert a call at the beginning of a fxn
	- macro to apply a macro to all fxns in an impl
	- then i can manually edit a few places when I want to gather data

## Per-car properties

Need to associate car length between driving and parking sims.

---> could store this in master Sim; after all, there will be some more permanentish stuff like agent/building/trip/owned car soon
	- but soon need to bundle things together and pass less params everywhere
- or stash in parking sim, transfer to driving, and back later

Wait, length of car affects parking pretty critically. A bunch of things plumb
around the precomputed front of the spot, used for drawing and for cars to line
up their front in the sim. I think we need to plumb the true start of the spot
and have a method to interpolate and pick the true front.

Now trying the other way -- plumbing back and forth. How do we represent this for parked cars?
- store Vehicle in ParkedCar
	- I remember there was some reason this refactor broke last time, but let's try it again
- and store ParkedCar's directly in ParkingLane

## Logging

Libraries will do it too -- that's fine

nice UI features:
- highlight what general area publishes a message
- filter by area
- remember where the log scroller is, even when hidden
- jump to end or beginning quickly
- start at the end
- show new messages in OSD briefly, then vanish
- wrap long lines

log crate is annoying -- cant initialize it, but also have something else hold
onto it. probably have to use lazy static. not even sure I'll use this implicit
style long-term -- when two sims are running side-by-side, might be very
necessary to plumb more log context anyway.

## Code layers

At some point, geometry was a separate layer from the graph base-layer of
map_model. That doesn't work -- we can't even reason about what turns logically
exist without operating on cleaned-up geometry.

Control used to be separate from map_model for similar "purity" reasons.
map_model was supposed to just be unbiased representation of the world, no
semantics on top. Except bus stops and routes crept it, and map edits lived
there. Separate control layer is just awkward.

## IDs

Should LaneID have LaneType bundled in for convenience? CarID and VehicleType?
