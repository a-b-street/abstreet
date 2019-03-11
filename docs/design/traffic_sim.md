# Traffic Simulation from scratch

The goal of this article is to explain the traffic simulation model that A/B Street uses. There's a large amount of traffic simulation research in academia, but the papers are often paywalled or require background knowledge. This article is meant to be accessible to anybody with a basic background in software engineering and high-school kinematics.

## Introduction

what is a traffic simulation?

a/b st is a game, needs performance (city scale -- X road segments, X intersections, X agents total), determinism, not complete realism
agent-based, rule out others
complex maps

cars, buses, bikes only (things on the road in a queue)

abst is a game, what's the essence of scarcity? contention at intersections, lanes restricting usage, parking. NOT modeling pedestrians queueing on a sidewalk, bc in practice, doesnt happen (except maybe around pike place or at festivals). not modeling bike racks at all -- in practice, can lock up within a block of destination without effort.

if modeling big highways, this wouldnt be great. but we're focused on intra-city seattle -- modeling phenomena like jam waves not so important. if the player does parking->bus lane and the bus moves faster, but more cars circle around looking for parking, then the model is sufficiently interesting to answer the questions i want. dont need to model stopping distance for that.



### Map model

lanes, turns, conflicting turns
lane-changing

## Disrete-time model

idea of agents sensing, planning, acting in env every X seconds
so what does a car care about? dont hit lead, stop for intersection if needed, obey speed limit and vehicle limit
	(aka go as fast as possible otherwise -- but maybe would want fuel efficiency or smooth accel)
lookahead (worst case analyses), cover multiple lane->turn->lanes maybe
some of the math

### Retrospective

1) It's fundamentally slow; there's lots of busy work. Cars in freeflow with nothing blocking them yet, or cars 

2) Figuring out acceleration in order to do something for the next tick is complicated.
- floating pt bugs, apply accel but make sure speed doesnt go negative or dist doesnt exceed end of lane if they were supposed to stop... wind up storing an intent of what they wanted to do, make corrections based on that. hacky.

3) The realism of having cars accel and deaccel doesnt really add much, and since the approach has silly assumptions anyway (slam on brakes and accelerator as much as possible), unrealistic

## Discrete-event model take 1: time-space intervals

things that were not finished / still hard:

- cover a short lane
- real quadratic distance over time was breaking stuff
- impossible accel/deaccel happened
- faster lead car made adjusting follower very hard

## Discrete-event model take 2

Let's try again, but even simpler and more incremental this time.

### v0: one car

Forget about speed, acceleration, and even multiple cars momentarily. What is a car's basic state machine? They enter a lane, travel across it, maybe stop and wait for the intersection, execute a turn through the intersection, and then enter the next lane. We could assign reasonable times for each of these -- crossing a lane takes lane_distance / min(road's speed limit, car's max speed) at minimum. Intersections could become responsible for telling cars when to move -- stop signs would keep a FIFO queue of waiting cars and wake up each car as the last one completes their turn, while traffic signals could wake up all relevant cars when the cycle changes.

### v1: queueing

Alright, but what about multiple cars? In one lane, they form a queue -- no over-taking or lane-changing. The FSM doesn't get much more complicated: a car enters a lane, spends at least the freeflow_time to cross it, and then either winds up front of the queue or behind somebody else. If they're in the front, similar logic from before applies -- except they first need to make sure the target lane they want to turn to has room. Maybe cars are already backed up all the way there. If so, they could just wait until that target lane has at least one car leave. When the car is queued behind another, they don't have anything interesting to do until they become the queue's lead vehicle.

Another way to understand this system is to picture every lane as having two parts -- the empty portion where cars cross in freeflow and a queue at the end. Cars have to pay a minimum amount of time to cross the lane, and then they wind up in the queue. The time to go from the end of the queue to the front requires crunching through the queue front-to-back, figuring out when each successive lead vehicle can start their turn and get out of the way.

### Drawing

An intermission -- we haven't pinned down exactly where cars are at some point in time, so how do we draw them? The idea for this DES model began without worrying about this too much -- when the map is zoomed out, individual cars are hard to see anyway; the player probably just wants to know roughly where lots of cars are moving and stuck waiting. This can be calculated easily at any time -- just count the number of cars in each queue and see if they're in the Crossing or Queued state.

But when zoomed in, we do want to draw individual cars at exact positions through time! Luckily, this isn't hard at all. The only change from the timestep model is that we have to process a queue at a time; we can't randomly query an individual car. This is fine from a performance perspective -- we almost always want to draw all cars on lanes visible on-screen anyway.

So how does it work? First consider the queue's lead vehicle. If they're Queued, then the front of the car must be at the end of the lane waiting to turn. If they're Crossing, then we can just linearly interpolate their front position from (0, lane_length) using the time-interval of their crossing and the current time. Then we consider the queue's second car. In an ideal world where they're the lead car, we do the same calculation based on Queued or Crossing state. But the second car is limited by the first. So as we process the queue, we track the bound -- a car's front position + the car's length + a fixed following distance of 1m. The second car might be farther back, or directly following the first and blocked by them. We just take min(ideal distance, bound), and repeat for the third car.

### v2: preventing discontinuities

There's an obvious problem happening when the lead vehicle of a queue leaves the queue -- everybody queued behind them suddenly jump forward. Discontinuities like this are of course unrealistic, but more importantly for A/B St's purpose, looks confusing to watch. So let's try a simple fix: when a lead car exits a queue, update its follower to know to cross the remaining distance properly. The follower might be Queued right behind the lead, or they might still be Crossing. If they're still Crossing, no worries -- they'll continue to smoothly Cross to the end of the lane. But if they're Queued, then reset the follower to Crossing, but instead make them cover a smaller distance -- (lane_length - lead car's length - FOLLOWING_DISTANCE, lane_length), using the usual min(lane speed limit, car's max speed). Since they're Queued, we know that's exactly where the follower must be.

This smoothness comes at a price -- instead of a car taking one event to cross a lane, it now might go through a bunch of Crossing states -- the lane's max capacity for vehicles, at worst. That's not so bad, and it's worth it though.

### v3: starting and stopping early

The basic traffic model is now in-place. As we add more elements of A/B Street in, we need one last tweak to the driving model. Cars don't always enter a lane at the beginning or exit at the very end. Cars sometimes start in the middle of a lane by exiting an adjacent parking spot. They sometimes end in the middle of a lane, by parking somewhere. And buses will idle in the middle of a lane, waiting at a bus stop for some amount of time.

When we update a car, so far we've never needed to calculate exact distances of everybody on the queue at that time. That's just for drawing. But now, we'll actually need those distances in two cases: when a car is finished parking or when a car is somewhere along their last lane. (Note that buses idling at a stop satisfy this second case -- when they leave the stop, they start following a new path to the next stop.) When the lead car vanishes from the driving lane (by shifting into the adjacent parking spot, for example), we simply update the follower to the Crossing state, starting at the exact position they are at that time (because we calculated it). If they were Queued behind the vanishing car, we know their exact position without having to calculate all of them. But Crossing cars still paying the minimum time to cross the lane might jump forward when the car in front vanishes. To prevent this, we refresh the Crossing state to explicitly start from where they became unblocked.

### Remaining work

There's one more discontinuity remaining. Since cars have length, they can occupy more than one lane at a time. When the lead car leaves a queue, the follower is updated to cross the remaining distance to the end. But if the leader is moving slowly through their turn, then the follower will actually hit the back end of the lead vehicle! We need a way to mark that the back of a vehicle is still in the queue. Maybe just tracking the back of cars would make more sense? But intersections need to know when a car has started a turn, and cars spawning on the next lane might care when the front (but not back) of a car is on the new lane. So maybe we just need to explicitly stick a car in multiple queues at a time and know when to update the follower on the old lane. Except knowing when the lead car has advanced some minimum distance into the new lane seemingly requires calculating exact distances frequently!

This jump bug also happens when a lead car vanishes at a border node. They vanish when their front hits the border, even though they should really only vanish when their back makes it through.

The other big thing to fix is blind retries. In almost all cases, we can calculate exactly when to update a car. Except for three:

1) Car initially spawning, but maybe not room to start. Describe the rules for that anyway.

2) Car on the last lane, but haven't reached end_distance yet. Tried a more accurate prediction thing, but it caused more events than a blind retry of 5s.

3) Cars waiting to turn, but not starting because target lane is full. Could register a dependency and get waked up when that queue's size falls below its max capacity. Could use this also for gridlock detection.

## A/B Street's full simulation architecture

start from step(), the master event queue, how each event is dispatched, each agent's states

FSM for intersections, cars, peds
(need to represent stuff like updating a follower, or being updated by a leader)

## Appendix

pedestrian, transit, bikes, buses, etc
limits... overtaking (especially cars and bikes on roads)
