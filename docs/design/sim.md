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
