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
