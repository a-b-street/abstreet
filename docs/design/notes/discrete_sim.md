# Discrete simulations

## Why are timesteps bad?

Discrete-time sim has two main problems:

1) It's fundamentally slow; there's lots of busy work.

2) Figuring out acceleration in order to do something for the next tick is complicated.

Discrete-event sim is the real dream. I know when a ped will reach the end of a
sidewalk and can cheaply interpolate between for drawing. If I could do the
same for all agents and states/actions, I could switch to discrete-event sim
and SUPER speed up all of the everything.

This possibly ties into the FSM revamp idea perfectly. Every state/action has
two durations associated with it:

- the best case time, calculated while pathfinding, which assumes no
  conflicts/delays due to scarcity
- the actual time something will take, based on state not known till execution
  time

## Intent-based kinematics

Instead of picking acceleration in order to achieve some goal, just state the
goals and magically set the distance/speed based on that. As long as it's
physically possible, why go through extra work?

Instead of returning accel, what if we return (dist to travel this tick, new speed).
- How can we sanity check that those choices are reasonable? Seems like we
  still have to sorta reason about acceleration properties of vehicles.
- But maybe not. each constraint...
	- dont hit lead... if we're ALREADY close to them (current speed * timestep puts us within follow_dist of them right now), just warp follow_dist away from where they are now.
	- if we need to stop for a point that's close, then we ideally want to do the kinda (do this distance, over this duration) thing and somehow not reason about what to do the next few ticks. event based approach for one thing. :\

Or different primitives, much closer to event sim...
- Often return (dist to wind up at, duration needed to get there)
	- linear interpolate position for rendering in between there
	- use for freeflow travel across most of a lane
	- to slow down and stop for a fixed point, do a new interval
	  (STOPPING_DISTANCE_CONSTANT, few seconds to stop). the vehicle will
	  kinda linearly slow down into the stop. as long as the stopping
	  distance and time is reasonable based on the vehicle and the speed
	  limit (freeflow vs shuffling forwards case?), then this'll look fine.
- What about following vehicles?
	- it'd be cool to have some freeflow movement to catch up to a lead vehicle, but if we look at the lead's position when we first analyze, we'll cover some ground, then replan again and do zenos paradox, planning freeflow for less new dist...
	- parked cars departing will have to interrupt existing plans!
	- rendering becomes expensive to do piecemeal; need to process cars in order on a queue to figure out positions. if we need to lookup a single one, might need to do a bunch of hops to figure out the front.
		- not really easy to percolate down a single time to pass the intersection from the lead vehicle... intersections need to stay pretty detailedish.

Lookahead constraints:
- go the speed limit
- dont hit lead vehicle (dist is limited by their dist + length + following dist)
- stop by a certain point
- future: do lane-changing stuff

Blah. Initial simplicity of this idea lost. Maybe try a single, simple hack
out: replace accel_to_follow with something to warp to the right spot behind an
agent and set the speed equal to min(follower's max speed, lead vehicle's
current speed).

- do we have a bug where vehicles can be too close when they're on adjacent but different traversables?
- if we just warp to follow_dist away from a vehicle when we get too close, then we instantly decelerate.

## The software engineering question

Is there a reasonable way to maintain both models and use one headless/editor
for them? Seemingly, the entire Sim API needs to just be abstracted. Like the
DrawAgents trait, but a bit more expanded. Except stuff like time travel
applied to discrete-time sim can't support debugging agents or showing their
path. I think time travel with discrete-event would work fine -- just storing
all the previous states with the exact times.

## What broke about the old simplified driving model?

- straw model has some quirks with queueing
	- after the lead vehicle starts the turn, the queue behind it magically warps to the front of the road
		- following distance needs to apply across lanes/turns -- lookahead has to still happen
	- the first vehicle in the turn jumps to a strange position based on the front/back rendering
		- dont remember what this was
- at signals, cars doing the same turn wont start it until the last car finishes it
		- dont remember what this was

## New thoughts on this

- picture each lane as the end of the slow queue and the end of the moving
  queue. spillback can happen. forget individual agents, just intervals
containing cars that expand/contract.

- dont need to calculate time to change speeds for arbitrary things. just time
  to go from 0->speed, speed->0. not anything else. in stop-and-start mode, do
something else -- the bound is on the intersection, not on the car to
accel/deaccel.

- when we apply the accel, check the intent -- if we overshoot, detect it
  early, maybe force slight correction. meaure how much correction we need.
	- interesting... the most constraining intent flips between FollowCar and StopAt.
	- the car winds up at the right dist, but with tiny speed! just pretend they managed zero?
