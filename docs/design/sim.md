# General simulation-related design notes

## Spawning agents ##

Ideally, each method would return a future that would do stuff, right? But the
mutations need to be done serially. So I think each sim's method should take
the path outright, not even start/end. Stick the rng work in sim for the
moment. This should let the start/goal selection and the parallelization of
paths happen at a more outer layer, in the sim aggregator.

... and now for scenarios / spawners. these get to run every step, trying to
introduce new things in the different simulations. if a parked car can't
currently begin departing, it'll keep trying every tick.


## Notes on determinism ##

- serde tricks
- unit tests

## Modeling choices

Don't model sidewalk crowdedness or bike rack availability, because in
practice, these things are never scarce resources or problematic. Finding a
parking spot is difficult and impacts the quality of one trip and causes
externality, so we should model that.

## Trips

Time to get even more multi-modal / multi-phase!

- all trips begin and end at a building
- spawn peds at a building, make them first traverse the front path.
	- could model another type of On
	- or, just have a special state in the walking sim, just like the driving sim has a temporary parking/unparking state

- the walking layer shouldnt care about the next layer of the trip. just tell
  master sim when a ped has reached a bldg or a parking spot, as desired.

- need to draw the FSM for all of this!


maybe need to organize structs/enums a little...

ParkingSpot
	- change this to just lane and spot idx, move other stuff to queries for ParkingSim, make it copyable
CarParking
	- rename to ParkedCar
SidewalkSpot
	- this should cache lane and distance. :)

## Scenarios

Awkward to turn neighborhoods into buildings/streets; we kind of need the
quadtree and stuff for that, which is the Renderable layer right now.
Originally there was a separate geometry layer, probably for stuff like this.

## Scores

Alright, getting much closer to this being a game! Let's return to the idea of utility functions for agents.

- everyone cares about total trip time
- everyone kind of cares about time spent waiting at intersections
- drivers (anybody using a car for part of their trip)
	- easiness of parking... partly this is time spent walking (from start bldg or to goal bldg), and partly time spent driving after initially reaching destination lane
- bikes (woops, not implemented yet :P)
	- climbing up hills
	- amount of time on busy roads
		- dedicated lanes are fine
		- even dedicated lanes too close to parking are bad -- stress from possibiliy of being doored
		- driving lanes with few actual cars passing are bad
- peds
	- hills up OR down
	- amount of greenery along route
	- amount of amenities like cafes along route
	- Seattle greenways had more factors that make a road pleasant or not

Per agent, this score is some kind of a linear combination of factors. Coefficients vary per agent -- some people like hills, don't care about busy roads, etc.

But let's start super simple: just track total trip time for all agents. What's the live UI view we want?

- per population type (peds, drivers), number of pending and completed trips. sum score so far (can use time so far for pending trips)
	- note that sum score alone is a bit meaningless, even between population types. need to A/B test to meaningfully compare.
- In headless mode, print scores at the end
- in UI, have an optional OSD to pop up on the right with scores so far

## Edit-Invariant

Back in the day, TurnID went from a number to (Lane, Lane, Intersection), so
that a single edit wouldn't totally throw off all the turn IDs. Now seeing a
similar problem when removing a parking lane -- the same 'seed parked cars'
command has vastly different effects between the same sim, and this tiny change
throws off the RNG and percolates everywhere. Let's think about how to make
more things invariant.

- should cars be the same?
	- maybe peds have to travel farther from home to start driving.
	- does the car on the far road belong to anyone in the other sim?
- forget debugability for a moment, what do we actually need to compare?
	- entire trips! maybe some use a bus or not btwn two worlds
	- warp to trip (using the active mode), compare trips
		- problem: ped IDs right now differ wildly because the rng gets offset. maybe those should always be chosen first? maybe they should be calculated independently and stuck in the Scenario? that part of a trip is a sim-invariant ID, a spawn time, and a start/goal building. the details (legs of trip) are calculated per sim.
- the RNG is also used after spawning to later roam around for parking
	- small road edits shouldnt affect this. per-car rng in an extreme, or maybe just an RNG for sim and for spawning?
		- but as soon as two cars have to wander for parking instead of one, everything gets offset completely.
- this will be harder later with spawners on map edges...

Alright, the deviations are still starting too early!
- Swapping shouldn't show different parked cars when the parking lane is present in both sims... or should it?
	- If 50% of spots in a neighborhood need to be initially populated, then more or less options for those DOES affect it.
		- the problem seems like "seed 50% of parked car spots in this area" is too vague. but theres also a desire that scenarios remain human-readable, high-level.
- Peds are picking very different parked cars in the neighborhood to go drive, causing big deviations early on.

Can we avoid storing trip/parked car mappings in the scenario?
- if peds pick the parked car closest to their start building... maybe

### Idea: forking RNGs

50% of spots filled isn't really accurate -- we're saying for every spot, flip
a coin with some weight. The flips are independent. We can regain an amount of
determinism by forking RNGs -- for every single lane in the map, use the main
RNG to generate a new RNG. Use that new RNG only if it really is a parking
lane.

Cool, much closer to working! Spots are consistently filled out or not. Car
colors are different, because car IDs are different. Making CarIDs consistent
would require changing them to have a stable form -- namely, their original
parking spot (lane ID and an offset). We _could_ do that...

But also wait, ped IDs are a bit different in some cases, and a trip is missing
entirely... huh?

Ah, we call gen_range on different inputs. Not sure why that throws off IDs
though... Can we fork RNG for that too?

Problems:
- CarIDs are different, could make them be original parking spot
= gen_range on different inputs

## Parked cars and ownership

Most cars are associated with a household. They usually park close to that
building (during overnight times). Some cars have no building -- buses, or cars
that might be visiting from outside the simulated area. A building might have
several cars.

Given:
- number of cars that should be around each house
- number of visiting cars
Can then seed parked cars kinda greedily.

But back up, other section.

Alright, now how do peds picking a car work?
- easy case: if the same car is present in both worlds, should use the same
- if cars are associated with buildings in a stable way, this should probably
  help or at least push the problem to a slightly more explicit place
- if a parking lane is gone and an agent wouldve used something there, of
  course they have to walk farther to get to their car. but we want to somehow
  localize these effects. same number of parked cars in the neighborhood and same
  assignment to buildings, but maybe some other lane gets more crowded?

Next thoughts:
- scrap the concept of SeedParkedCars. instead say "x% of buildings in this neighborhood have 1 or 2 cars." peds from these buildings will use those specific cars, others wont.
	- probability of 0 cars = 40, 1 car = 40, 2 cars = 20   <--- thats much nicer
- go through each building, find a spot to seed that parked car.
	- try to be close to building, random jitter to be a little far away
	- if this process is somewhat stable, then should be fine. doesnt need to be perfectly stable -- removing lanes somewhere might put more pressure on another lane.

## Traces between worlds

Alright, now how do we even compare trip progress to show it visually? This is
kind of a UI-only problem; total score at the end can be compared way more
easily.

- easy case: if both worlds are using the same mode AND route, trace_route the
  slower agent with the dist_ahead of the faster agent (difference between the
  two)
	- alright, have to track current distance traveled to do this. can keep
	  state per trip leg or something.
- if it's unclear who's closer to the goal, could just pathfind to exactly
  where the other agent is, display in a neutral way
	- mode changes are potentially weird... but just slide over between sidewalks and driving lanes? mmm...
- strawman: dont use the trace route thing, just have a straight arrow to the
  other agent and green/red based on straight-line distance to goal bldg
- progress is non-monotonic -- might walk away from goal to get to car, then get there faster. or maybe get stuck in traffic. so straightline distance to goal is EXPECTED to fluctuate. thats kind of exciting to watch anyway.

Ah, upon seeing just the line between, I know what would make more sense --
show the divergence. The point in the route where one version goes one way, and
the second goes another. Two routes shown, symmetric.
