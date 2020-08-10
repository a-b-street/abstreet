# Traffic Simulation from scratch

The goal of this article is to explain the traffic simulation model that A/B
Street uses. There's a large amount of traffic simulation research in academia,
but the papers are often paywalled or require background knowledge. This article
is meant to be accessible to anybody with a basic background in software
engineering and high-school kinematics.

Disclaimers... my background is in software engineering, not civil engineering.
The design space that A/B Street explores is absolutely massive; there are so
many alternate ways of doing everything from modeling the map, to representing
agents and their movement and conflict... Please send any critique/feedback/etc

## Introduction

My other goal with this article is to explain what I work on to my friends and
family. It's very easy to say "I'm not a computer programmer or a math expert,
there's no way I could understand this," and despite how frustrating this is,
I've previously accepted this. But today I want more. Driving, moving around a
city, and getting stuck in traffic are common experiences, and I think they can
help...

Imagine there's an incredibly rainy afternoon and we've got lots of paper. I
draw a to-scale map of your hometown, showing details like how many lanes are on
every road, noting the speed limits, and marking all the stop signs and traffic
lights. Then I place a bunch of color-coded Hot Wheels and, I don't know, bits
of paper around the map. Each of the cars will start somewhere and wants to go
to their colored square. To make it easy, let's pretend they all start driving
at the same time. My challenge for you is to show me exactly where the cars are
30 seconds after starting to drive, then 5 minutes in, and then an hour later.
Maybe I'm interested in figuring out where the traffic jams happen. Or maybe we
throw in some buses and little toy soldiers, and I want to know how long people
after waiting for their bus because it's delayed in traffic. Or maybe I'm just
sadistic and want to watch you squirm.

How would you figure out what happens to all of the cars after some amount of
time? You'll probably start by figuring out the route each of them will take to
their destination -- probably some approximation of the shortest path (by pure
distance) or fastest (a longer route on a highway might be faster than a short
one through residential roads). You'll inch the cars forward on their lane, not
moving them (too much) faster than the speed limit. When two cars are near each
other, you'll make one follow the other at a reasonable distance, or maybe
change lanes and try to overtake them if there's room. You'll make the cars stop
at stop signs and for red lights. When you accidentally push too many cars
through a green light turned feisty yellow then somber red, you'll make the
opposing lane's cars angrily honk at the jerk blocking the box by making odd
little squeaks out of the corner of your mouth. (And I will laugh at you, of
course.)

Of course, you won't be able to tell me with perfect accuracy where all the cars
are 45.2 seconds into our little game. There are potholes that'll slow some cars
down a bit that aren't marked on the map, and some drivers that take a little
longer to accelerate or notice the light's green. That's fine -- complete
realism isn't so important, as long as things look reasonable.

For this to be interesting for me to watch, there have to be a realistic number
of cars -- 10 little Hot Wheels squeaking around all of Seattle won't tell me
anything interesting about how the city flows. By now, you might be thinking
this is going to be slightly tedious. Your fingers are going to get a bit
cramped from budging 500,000 cars around a bit at a time. So I'll cut you a deal
-- if you'll describe rules for how to move each of the cars forward a bit in
sufficient detail, then I'll make a computer do all of the tedious bits.

And that's all programming a traffic simulator is. You don't need to know what
arrays and entity-component systems and trans-finite agent-based cellular RAM
drives are (I made up that last one maybe). Let's get started!

## The map

Let's start with deciding exactly what our map of Seattle looks like. One of the
trickiest and most satisfying parts about computer programming is figuring out
what parts of the world to represent. Too much irrelevant detail makes it harder
to... Yes, a tree partly blocking a tight corner might make people slow down,
but it's probably a bit too much detail to worry about. Your choice of
abstraction should, it turns out, depend on what you're actually trying to do.
In this case, I'll cheat momentarily and describe how we should model the map.
Later, I'll explain what I want A/B Street to be and how that led to including
some things while omitting others.

Let's also clear up terminology. Diagram goes here...

Let's start with **roads**. A road goes between exactly two **intersections**.
You might think of 2nd Ave as a long road through all of downtown, but we'll
chop it up as 2nd Ave between Bell St and Lenora, 2nd Ave from Lenora to Seneca,
etc. Most intersections have two or more roads connected to them, but of course,
we might also have dead-ends and cul-de-sacs. Each road has individual **lanes**
going in some direction. Most roads have lanes going both directions, but a few
one-ways only have lanes going in one direction. Cars will travel along a lane
in single file and, to keep things simple, never change lanes in the middle of a
road. When the car reaches the end of a lane, it can perform one of several
**turns** through the intersection. After finishing the turn, the car will be at
the beginning of a lane in another road. Some turns conflict, meaning it's not
safe for two cars to do them simultaneously, while others don't conflict.

If cars can't ever change lanes, couldn't they get stuck? Maybe a car starts on
the rightmost lane and is only allowed to turn right, but actually needs to be
in a middle lane to go straight through the intersection. Don't worry -- you can
assume that there's a path between any two lanes. Instead of changing lanes in
the middle of a road, cars in our game will change lanes when they turn. EXAMPLE
PIC. I'll describe later why this is a good idea.

For now, let's assume cars start on some lane. When their front bumper hits
their colored square on their destination, they just immediately vanish. The
colored square could be at the end of their destination lane, or somewhere in
the middle.

(could mention borders or not, maybe footnote)

Don't worry about parking, pedestrians, bicycles, or buses. These things are all
important to A/B Street, but we'll add them in later.

## Disrete-time model

Whoa, fancy name! Ignore it for a moment.

How do people drive? Very roughly, they look at things around them, take an
action (press the gas some amount, press the brake some amount, turn the wheel a
bit), and then do the same thing a half-second (or so) later. That's the essence
of agent-based modeling -- sense the environment, plan what to do next, do it,
then repeat some time later. We'll call the amount of time between each choice
the **timestep** and say it's about 0.1 seconds. Let's try simulating traffic
roughly this way -- every single car will take an action every 0.1 seconds that
advances them through the world. Breaking up time in these regular 0.1s
intervals is how we get the term "discrete-time model."

What kind of controls do we want to give each driver? If we let them turn the
steering wheel a few degrees left or right and apply some pressure to the gas
pedal, then we have to figure out how this affects the position of the car and
worry about how to make sure cars stay in their lane. That's way too
complicated, and not interesting for our purposes. So let's say that for every
car, we keep track of a few details:

- current lane or turn
- **dist_along**: distance of the front bumper along that lane or turn (starting
  at 0 for the beginning of the lane)
- current speed
- the remaining path to the goal (lane1, turn2, lane3, turn5, ..., lane10)
- vehicle length
- maximum acceleration (some cars can start more quickly)
- maximum deceleration (some cars can slam on their brakes and stop faster)

The first four will change every 0.1s, while the last three don't ever change.

So what controls can a driver do? Accelerate (or decelerate) some amount. That's
all. When the dist_along is bigger than the current lane/turn's length, we make
the new current lane be the first thing from the remaining path, discard the
path, and reset the dist_along to 0.

(this section got weird -- talk about controls first, brief bit of kinematics,
then state. follow along curve of lanes automatically.)

### Constraints

What kind of things influence a driver's decision each timestep, and what do
they need to be able to sense about their environment to use the rule? I can
think of three:

1. Don't exceed the speed limit of the current road

- so the driver needs to be able to look at the speed limit of the current road

2. Don't hit the car in front of me

- need to see the current dist_along, speed, length, accel and deaccel of the
  next car in the queue
- actually, humans can't eyeball another car and know how quickly it can speed
  up or slow down. maybe they just assume some reasonable safe estimate.

3. Maybe stop at the end of the lane

- for stop signs or red/yellow lights

And of course, whatever acceleration the driver picks gets clamped by their
physical limits. Other than these constraints, let's assume every driver will
want to go as fast as possible and isn't trying to drive smoothly or in a
fuel-efficient manner. Not realistic, but makes our lives easier. So if each of
these three constraints gives an acceleration, we have to pick the smallest one
to be safe. Rule 1 says hit the gas and go 5m/s^2, but rule 2 says we can safely
go 2m/s^2 and not hit the next car and rule 3 actually says wait, we need to hit
the brakes and go -1m/s^2 right now. Have to go with rule 3.

### Some math

Skip this section freely. The takeaways are:

- there's a way to figure out the acceleration to obey the 3 constraints
- the math gets tricky because (1) the car will only be doing that acceleration
  for 0.1s and then getting to act again, and (2) floating point math is tricky

### Lookahead

lookahead (worst case analyses), cover multiple lane->turn->lanes maybe

### Retrospective

1. It's fundamentally slow; there's lots of busy work. Cars in freeflow with
   nothing blocking them yet, or cars

2. Figuring out acceleration in order to do something for the next tick is
   complicated.

- floating pt bugs, apply accel but make sure speed doesnt go negative or dist
  doesnt exceed end of lane if they were supposed to stop... wind up storing an
  intent of what they wanted to do, make corrections based on that. hacky.

3. The realism of having cars accel and deaccel doesnt really add much, and
   since the approach has silly assumptions anyway (slam on brakes and
   accelerator as much as possible), unrealistic

## Discrete-event model take 1: time-space intervals

things that were not finished / still hard:

- cover a short lane
- real quadratic distance over time was breaking stuff
- impossible accel/deaccel happened
- faster lead car made adjusting follower very hard

## Discrete-event model take 2

Let's try again, but even simpler and more incremental this time.

(make sure to explain the premise of parametric time and events)

### v0: one car

Forget about speed, acceleration, and even multiple cars momentarily. What is a
car's basic state machine? They enter a lane, travel across it, maybe stop and
wait for the intersection, execute a turn through the intersection, and then
enter the next lane. We could assign reasonable times for each of these --
crossing a lane takes lane_distance / min(road's speed limit, car's max speed)
at minimum. Intersections could become responsible for telling cars when to move
-- stop signs would keep a FIFO queue of waiting cars and wake up each car as
the last one completes their turn, while traffic signals could wake up all
relevant cars when the cycle changes.

### v1: queueing

Alright, but what about multiple cars? In one lane, they form a queue -- no
over-taking or lane-changing. The FSM doesn't get much more complicated: a car
enters a lane, spends at least the freeflow_time to cross it, and then either
winds up front of the queue or behind somebody else. If they're in the front,
similar logic from before applies -- except they first need to make sure the
target lane they want to turn to has room. Maybe cars are already backed up all
the way there. If so, they could just wait until that target lane has at least
one car leave. When the car is queued behind another, they don't have anything
interesting to do until they become the queue's lead vehicle.

Another way to understand this system is to picture every lane as having two
parts -- the empty portion where cars cross in freeflow and a queue at the end.
Cars have to pay a minimum amount of time to cross the lane, and then they wind
up in the queue. The time to go from the end of the queue to the front requires
crunching through the queue front-to-back, figuring out when each successive
lead vehicle can start their turn and get out of the way.

### Drawing

An intermission -- we haven't pinned down exactly where cars are at some point
in time, so how do we draw them? The idea for this DES model began without
worrying about this too much -- when the map is zoomed out, individual cars are
hard to see anyway; the player probably just wants to know roughly where lots of
cars are moving and stuck waiting. This can be calculated easily at any time --
just count the number of cars in each queue and see if they're in the Crossing
or Queued state.

But when zoomed in, we do want to draw individual cars at exact positions
through time! Luckily, this isn't hard at all. The only change from the timestep
model is that we have to process a queue at a time; we can't randomly query an
individual car. This is fine from a performance perspective -- we almost always
want to draw all cars on lanes visible on-screen anyway.

So how does it work? First consider the queue's lead vehicle. If they're Queued,
then the front of the car must be at the end of the lane waiting to turn. If
they're Crossing, then we can just linearly interpolate their front position
from (0, lane_length) using the time-interval of their crossing and the current
time. Then we consider the queue's second car. In an ideal world where they're
the lead car, we do the same calculation based on Queued or Crossing state. But
the second car is limited by the first. So as we process the queue, we track the
bound -- a car's front position + the car's length + a fixed following distance
of 1m. The second car might be farther back, or directly following the first and
blocked by them. We just take min(ideal distance, bound), and repeat for the
third car.

### v2: preventing discontinuities

There's an obvious problem happening when the lead vehicle of a queue leaves the
queue -- everybody queued behind them suddenly jump forward. Discontinuities
like this are of course unrealistic, but more importantly for A/B St's purpose,
looks confusing to watch. So let's try a simple fix: when a lead car exits a
queue, update its follower to know to cross the remaining distance properly. The
follower might be Queued right behind the lead, or they might still be Crossing.
If they're still Crossing, no worries -- they'll continue to smoothly Cross to
the end of the lane. But if they're Queued, then reset the follower to Crossing,
but instead make them cover a smaller distance -- (lane_length - lead car's
length - FOLLOWING_DISTANCE, lane_length), using the usual min(lane speed limit,
car's max speed). Since they're Queued, we know that's exactly where the
follower must be.

This smoothness comes at a price -- instead of a car taking one event to cross a
lane, it now might go through a bunch of Crossing states -- the lane's max
capacity for vehicles, at worst. That's not so bad, and it's worth it though.

### v3: starting and stopping early

The basic traffic model is now in-place. As we add more elements of A/B Street
in, we need one last tweak to the driving model. Cars don't always enter a lane
at the beginning or exit at the very end. Cars sometimes start in the middle of
a lane by exiting an adjacent parking spot. They sometimes end in the middle of
a lane, by parking somewhere. And buses will idle in the middle of a lane,
waiting at a bus stop for some amount of time.

When we update a car, so far we've never needed to calculate exact distances of
everybody on the queue at that time. That's just for drawing. But now, we'll
actually need those distances in two cases: when a car is finished parking or
when a car is somewhere along their last lane. (Note that buses idling at a stop
satisfy this second case -- when they leave the stop, they start following a new
path to the next stop.) When the lead car vanishes from the driving lane (by
shifting into the adjacent parking spot, for example), we simply update the
follower to the Crossing state, starting at the exact position they are at that
time (because we calculated it). If they were Queued behind the vanishing car,
we know their exact position without having to calculate all of them. But
Crossing cars still paying the minimum time to cross the lane might jump forward
when the car in front vanishes. To prevent this, we refresh the Crossing state
to explicitly start from where they became unblocked.

### Remaining work

There's one more discontinuity remaining. Since cars have length, they can
occupy more than one lane at a time. When the lead car leaves a queue, the
follower is updated to cross the remaining distance to the end. But if the
leader is moving slowly through their turn, then the follower will actually hit
the back end of the lead vehicle! We need a way to mark that the back of a
vehicle is still in the queue. Maybe just tracking the back of cars would make
more sense? But intersections need to know when a car has started a turn, and
cars spawning on the next lane might care when the front (but not back) of a car
is on the new lane. So maybe we just need to explicitly stick a car in multiple
queues at a time and know when to update the follower on the old lane. Except
knowing when the lead car has advanced some minimum distance into the new lane
seemingly requires calculating exact distances frequently!

This jump bug also happens when a lead car vanishes at a border node. They
vanish when their front hits the border, even though they should really only
vanish when their back makes it through.

The other big thing to fix is blind retries. In almost all cases, we can
calculate exactly when to update a car. Except for three:

1. Car initially spawning, but maybe not room to start. Describe the rules for
   that anyway.

2. Car on the last lane, but haven't reached end_distance yet. Tried a more
   accurate prediction thing, but it caused more events than a blind retry of
   5s.

3. Cars waiting to turn, but not starting because target lane is full. Could
   register a dependency and get waked up when that queue's size falls below its
   max capacity. Could use this also for gridlock detection. Oops, but can't
   start because of no room_at_end. That's different than target lane being
   full, and it's hard to predict when there'll be room to inch forward at the
   end.

#### Notes on fixing the jump bug

- can track a laggy leader per queue. there's 0 or 1 of these, impossible to be
  more.
- a car can be a laggy leader in several queues, like long bus on short lanes.
  last_steps is kinda equivalent to this.

is it possible for get_car_positions to think something should still be blocked
but the rest of the state machine to not? - dont change somebody to
WaitingToAdvance until theres no laggy leader.

TODO impl:

- get_car_positions needs to recurse
- trim_last_steps needs to do something different

the eager impl:

- anytime we update a Car with last_steps, try to go erase those. need full
  distances. when we successfully erase, update followers that are Queued. - -
  follower only starts moving once the laggy lead is totally out. wrong. they
  were Queued, so immediately promote them to WaitingToAdvance. smoothly draw in
  the meantime by recursing
- optimistic check in the middle of Crossing, but use a different event for this
  to be clear. the retry should only be expensive in gridlocky cases. -
  BLIND_RETRY after... similar to end_dist checking. - note other routines dont
  even do checks that could hit numerical precision, we just assume events are
  scheduled for correct time.
- maybe v1: dont recurse in get_car_positions, just block off entire laggy
  leader length until they're totally out.
- v2: do have to recurse. :(

the lazy impl:

- when we become WaitingToAdvance on a queue of length > vehicle len, clean up
  last_steps if needed
- when we vanish, clean up all last_steps
- when a follower gets to ithin laggy leader length of the queue end, need to
  resolve earlier - resolving early needs to get exact distances. expensive, but
  common case is theyre clear. if somebody gets stuck fully leaving a queue,
  then this will spam, but thats also gridlocky and we want to avoid that anyway

other ideas:

- can we cheat and ensure that we only clip into laggy leaders briefly? - if
  laggy leaders never get stuck at end of their next queue...
- alt: store front and back of each car in queues separately - compute crossing
  state and stuff for those individually? double the work?

## A/B Street's full simulation architecture

start from step(), the master event queue, how each event is dispatched, each
agent's states

FSM for intersections, cars, peds (need to represent stuff like updating a
follower, or being updated by a leader)

## Appendix

pedestrian, transit, bikes, buses, etc limits... overtaking (especially cars and
bikes on roads)

Traffic modeling is a complex space, but for the purposes of this article, a
traffic simulation is a computer program that takes a map with roads and
intersections and a list of trips (depart from here at this time, go there using
a car or bus or by foot) and shows where all of the moving agents wind up over
time. I'm sure you can imagine a great many uses for them both professional and
nefarious, but our mission today is to understand one particular traffic
simulation works. A/B Street is a computer game I've been building to experiment
with the traffic in Seattle. My goal for A/B Street is to make it easy for
anybody to ask what-if questions.

a/b st is a game, needs performance (city scale -- X road segments, X
intersections, X agents total), determinism, not complete realism agent-based,
rule out others complex maps

cars, buses, bikes only (things on the road in a queue)

abst is a game, what's the essence of scarcity? contention at intersections,
lanes restricting usage, parking. NOT modeling pedestrians queueing on a
sidewalk, bc in practice, doesnt happen (except maybe around pike place or at
festivals). not modeling bike racks at all -- in practice, can lock up within a
block of destination without effort.

if modeling big highways, this wouldnt be great. but we're focused on intra-city
seattle -- modeling phenomena like jam waves not so important. if the player
does parking->bus lane and the bus moves faster, but more cars circle around
looking for parking, then the model is sufficiently interesting to answer the
questions i want. dont need to model stopping distance for that.
