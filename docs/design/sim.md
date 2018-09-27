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
