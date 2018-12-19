# Driving-related design notes

## Strawman driving model

- Show the FSM
- Explain how the model is based on best-case bounds
- Position is derived lazily from time
- How accurate could it be? Based on inner-city speeds and timesteps

- problems
	- straw model has some quirks with queueing
		- after the lead vehicle starts the turn, the queue behind it magically warps to the front of the road
		- the first vehicle in the turn jumps to a strange position based on the front/back rendering
	- at signals, cars doing the same turn wont start it until the last car finishes it

## AORTA driving model ##

- figure out how to dynamically sub out two different driving models. traits or generics on Sim.
	- repeat determinism tests for both!
- start sharing code between the two models (and the ped model)

the polymorphism mess:
	- a sim can contain one of two different driving models
	- driving models both implement the same interface (a trait)
	- need to serialize/deserialize them, but cant get deserialization with erased_serde working
	- so use enums and just delegate everything. macro?
	- tried deref on an enum, returning a trait
	- delegate crate can't handle match expressions

## The lookahead exceeding-speed-limit bug

It happens because a car was previously throttling itself due to somebody in
the way, but as soon as they start a turn, the car eagerly jumps ahead.

ah no, it's because we use max_lookahead_dist in accel_to_follow, and the speed limit we assert is the old road's speed.

## Routers, lookahead, cars as FSMs

Hmm, hard to figure out the interactions for the router. when should it get a
chance to roam around for parking? should lookahead make copies of it (harder
to then roam...) or should it index into one of them (preferred...)


rethink cars as a much more careful FSM. on a lane, on a turn, on the LAST lane
lining up for something, parking, unparking, idling at a bus stop. these states
imply things like position (which queue to occupy space in). both lookahead and
physically stepping forwards ought to be able to use the same code -- the
routing, turn<->lane transitions, leftover dist calculation, etc shouldnt care
if its real or projected


Try this again, but with a much more careful interface for the router. But when do we get to mutate?

- during react() when we up-front see the path is empty and theres no parking
	- problem: what if lookahead earlier spots this case and doesnt know where to go?
	- soln: who cares. when that happens, just recommend stopping at the
	  end of the lane. when we physically get there, this case will trigger
          and we can mutate.
- during react() lookahead when we first try to go beyond the end
	- problem: but then it's hard to actually do the lookahead; we cant
	  clone the router and transition it along during lookahead. we would
	  have to go update the original one somehow.
- in react() lookahead, return an indicator and then have step() call another mutable method later.
	- this is the confusing kind of split I'm trying to avoid.

So I think the first solution works best.


Urgh, we have a mutability mess again -- react() needs a mutable router, but
immutable view into world state to query sim queues and find next car in front
of. Similar to needing to know speed and leader vehicles for intersection sim.
For now, should we do the same hack of copying things around?
	- kinda awkward that routers and cars are separate; a router belongs to
	  a car. maintaining the mappings in the same spots is gross. need to
          express in react() that only one part of a car -- the router/plan -- is
          mutable.
	- it's also horrible that DrivingSimState is passed around to SimQueue to get car positions!
		- an alt is to duplicate state into the simqueue and store the dist along. this would be pretty easy actually...
	- and passing around properties / all the vehicles is awkward
	- maybe it's time to cave and try an ECS?
		- just start by listing out what needs to happen.
		- car react needs mutable router, immutable queues of lanes and turns, immutable dist/speed of other cars
		- intersections need to know car speed, whether cars are leaders (so simqueue positions)

Short term solution: copy a huge nasty thing and pass it into react(). :\
	- but I'm still not convinced any mutabulity in react() is good
