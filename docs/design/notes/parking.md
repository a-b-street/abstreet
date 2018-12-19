# Parking-related design notes

## Parking

- already drawing parking spots of some length
- car has to drive on adjacent driving lane past that distance, then spend X seconds parking or unparking
	- draw different color while doing this
	- this will probably mess up the clunky minimal driving model that infers distance based on time
- Need to mark occupancy of all the parking spots. should be there for parking lanes instead of SimQueue.
- Start the sim with a bunch of parked cars
	- how to model those cars? not as active agents.
	- no more spawning a car! select a car to wake up. :D

The car's FSM:

```dot
parked -> departing;
departing -> traveling_along_road;
traveling_along_road -> waiting_for_turn;
waiting_for_turn -> executing_turn;
executing_turn -> traveling_along_road;
traveling_along_road -> parking;
parking -> parkd;
```

- I guess CarIDs are going to be handled a little differently now; all the cars will be created once up-front in a parking state
- Don't really want active Car structs for all the parked cars. Or, don't want to ask them to do stuff every tick.
	- As we add more agent types, it'll become more clear how to arrange things...
	- But for now, make something to manage both active and parked cars.

- Kind of seeing two designs
	- master sim owns driving and parking state. a CarID is managed by exactly one. master sim has to enforce that.
	- master sim owns car state as an enum, calls high-level step-forward functions for driving and parking
		- perf: cant iterate just the active cars?

How to represent departing/parking states?
- could have state in both driving and parking sims. hacks to make driving not advance the car state.
- could represent it in the master sim state, but that's also a slight hack
---> or, own it in the driving state, since thats the major place where we need to block other cars and make sure we dont hit things.
	- should we tell parking state about the transitional cars or not? driving should render them. might make statistics and looking for free spots weird, but let's not tell parking about them yet!

Time to implement roaming if there are no spots free!
