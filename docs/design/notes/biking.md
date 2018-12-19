# Biking-related design notes

## Bike lanes

How do we model bikes merging to a driving lane to make a left?

## General modeling

Bikes are just like cars and can use all the same code, right? Except...

- different speeds
- can use bike or driving lanes
- they dont park or unpark
	- actually, how does a ped start/stop using a bike?
	- modeling bike lockup locations is overkill. in practice not a
	  problem. fixed 60s penalty to start/stop biking. penalty happens on
	  the sidewalk, then they spawn onto the bike/driving lane at the very end
- they can _maybe_ overtake on a bike lane?
	- nah, that should be modeled as two bike lanes (or a bike and driving lane) and normal lanechanging behavior
- different rendering
- dont want to use CarID; could maybe rename it VehicleID, but then bike-specific code (like rendering) gets weird

Because of the orchestration with walking/biking models, I'm tempted to try to share some common code, but keep things a bit separate. However, if they're separate...

- driving lookahead needs to see bikes and vice versa
	- do the SimQueues need to be state that both models can access?


Do this in a branch for sure. Roughly:
- introduce BikeID, the rendering, stubs for spawning
- lift SimQueues into Sim
- refactor lookahead
- add biking model


After starting this, I'm not sure now. The driving model as-is can handle bikes
fine. The interaction with the walking sim to appear/disappear is pretty
minimal. Alternate idea for another branch:
- keep existing driving code almost entirely as is.
	= Vehicle bike type with super slow speed, modify driving lookahead to use the cap speed.
	= no BikeID, just a bit in Car for is_bike.
	= spawn param to decide if a trip without an owned car will instead bike
	= walking state can own the 'parking/unparking' state.
	= need a new DrivingGoal, simpler than ParkNear.
	= render peds doing bike prep differently
	= entirely new render code, but the same DrawCarInput (plus is_bike
	  bit). that part shouldn't matter, right?
	= make sure biking from border works, needs an extra bit i think

	- lastly: rename. Car -> Vehicle? Vehicle -> VehicleParams? DrivingSim -> QueuedSim?

	- etc
		= calculate_paths in spawn needs plumbing. introduce a PathfindingRequest struct, avoid those bools.

		- stats; driving.count and trip score
		- vehicle enum instead of is_bus, is_bike
			- put this in vehicle properties, not on the main
			  car... then dont need it in Command::DriveFromBorder.
		- spawn commands getting to have lots of similarish cases

		- remove the sim helpers that do specific stuff... think of
		  another way to set up tests, similar to tutorial mode?
		- verify abtest consistency
		- all of the get_blah_from_blah queries in map are a mess
		- Position(lane, dist) type would help, yeah?
		- a big help: get rid of dimensioned. make Eq work by wrapping
		  NotNaN or something else, maybe even requiring explicit
		  tolerance thing? get rid of all the terrible PartialEq hacks.
		- animate the bike preparation thing better... visually show time left somehow
