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
	- the car winds up at the right dist, but with tiny speed! just pretend they managed zero

## Time-space intervals

An interval is a car traveling along a lane from dist1 to dist2, time1 to
time2, speed1 to speed2. Within these intervals, those values (distance and
speed) are just linearly interpolated. Or it could be more detailed, vf = v0 +
at, and xf = x0 + v0t + 0.5at^2.

These intervals get created a few different ways...
- accelerate from rest to min(speed limit, vehicle's max speed)... need to
  calculate distance traveled during that time and time it takes
- opposite thing, deaccelerate
- freeflow travel... speed stays the same. can construct given desired time or desired distance.

Say a single car wants to just cover a lane and stop at the end. First accel as
much as possible. If distance is too much, then... there are actually a whole
space of options. Optimization problem. Whatever, pick some bad solution,
improve it later.

### Following

Do two cars conflict on a lane? If they have timespace intervals that overlap
(distance and time), then maybe. But they might not -- maybe a slow car barely
catches up to a faster car. We have two x(t) parametric equations -- set them
equal, find the time at which the collision occurs, see if that time is
actually in both time ranges. We actually want to check if x1(t) + length +
follow dist = x2(t), of course.

If there is a collision, make a new time-space interval for the follower. It
shouldn't need to reference the leader at all after making the interval!
Specifically...

leader:  (x1, t1)             (x2, t2)
follower:      (x3, t3,              x4, t4)

"collision" (with following distance added) at t5 and x5, which are inside the
appropriate intervals. the follower's intervals should now be...

(x3, t3    x5, t5) and (x5, t5,      ????)

Position should be the same as leader, but + the length and follow dist.
Correspondingly, the time to reach the same endpoint as the leader should be
offset by some amount...

- We sort of hand-wave assume that the follower can decelerate to match the
  speed of the leader.

### Stop-and-go conditions

What if we measured distance over time of this using the current sim? And then
found a low speed that produced the same distance in the same time?

### Gluing together traversables

When we "lookahead", now we just have to call maybe_follow against a single
vehicle. Need an easy way to indicate relative distance behind them.

## Event-based model, redux

Key idea to this one: stop insisting on knowing where individual cars are at all times.

A car's state machine:

- they enter a lane
- they cross it during some time... length / min(speed limit, their max speed)
- now they're in the queue!
	- if they're not first, then WAIT.
- now they're at the front of the queue
	- if the target lane is at capacity (queue + freeflowers == rest capacity), then WAIT.
		- could detect gridlock by looking for dependency cycles here
	- ask the intersection when they can go. tell it how long crossing will take (based on freeflow times again)
		- intersections know when conflicting turns will finish
		- stop sign will just give a little delay to some directions
		- traffic signal bit more complex (ohhh but no more overtime!!!)
		- probably want to add slight delay to avoid a packet of cars surging forward on a green light change
- cross the turn
- repeat!

How could lane-changing work?

- equivalent to existing: queue per lane, but no lanechanging
- simple: capacity based on all lanes, but one queue for a road. any turn.
	- bike/bus lanes don't help anything
	- one (usually turn) lane backfilling affects all
- later: queue per lane. turn becomes lane->road
	- the LCing itself doesnt really happen, merging has to happen upstream
- actual LCing during freeflow/queueing time could make sense

Now the interesting part... drawing!

- unzoomed
	- draw red queue slice at the end of a road, and a blue freeflow slice at the start
		- show some kind of movement in the blue?
	- while turns are being executed, just draw blue slice of the entire turn?
		- since they're individual, could animate by interpolating or something
- zoomed
	- hmm. can probably figure out exact car positions somehow, but not important.

This model neglects:

- speed, acceleration at some particular time
- but delays to doing turns after queueing could include time to accelerate

## Article on traffic simulation

- introduce problem, macroscopic out of scope
- discrete time... AORTA model
	- drawbacks
- time-space intervals
	- retrospective
- simpler discrete-event system
	- essence of scarcity
