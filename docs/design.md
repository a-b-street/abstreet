# Design notes

## Associated data / ECS

So far, the different structures for representing everything associated with a
road/intersection/etc strongly resembles ECS. Would explicitly using an ECS
library help?

http://www.gameprogrammingpatterns.com/component.html

Road has different representations:

- protobuf
- runtime map_model (mostly focusing on the graph)
- UI wrapper + geometry for simulation (should probably tease this apart)
- "control" layer for editable policies
- Queue of cars on the road

It could be useful to bundle together a context-like object of Map, GeomMap,
ControlMap, DrawMap, etc.

Need to figure out how to handle reaping old IDs for transient objects like
cars, but also things like modified roads. Slot maps?

## Immediate mode GUI

Things organically wound up implementing this pattern. ui.rs is meant to just
be the glue between all the plugins and things, but color logic particularly is
leaking badly into there right now.

## Strawman driving model

- Show the FSM
- Explain how the model is based on best-case bounds
- Position is derived lazily from time
- How accurate could it be? Based on inner-city speeds and timesteps

- problems
	- straw model has some quirks with queueing
		- after the lead vehicle starts the turn, the queue behind it magically warps to the front of the road
		- the first vehicle in the turn jumps to a strange position based on the front/back rendering
	- at signals, cars doing the same turn wont start it until the last car finishes it

## Stop sign editor

Stop signs are FIFO, except that many intersections only have a stop sign for
some sides. Going straight on the priority roads is immedite, and left turns
from those priority roads also take precedence over the low-priority roads. So
should the stop sign controller mark individual turns as priority/not, or
individual roads, with implied semantics for left turns? There are really 3
priorities if turns are considered...

Figuring out nonconflicting roads seems tricky. For now, going to have a
complicated UI and let individual turns be classified into 3 priority classes.
First group can't conflict, second and third groups can conflict and are FIFO.
Will probably have to revisit this later.

## UI plugins

- Things like steepness visualizer used to just be UI-less logic, making it
  easy to understand and test. Maybe the sim_ctrl pattern is nicer? A light
adapter to control the thing from the UI? ezgui's textbox and menu are similar
-- no rendering, some input handling.

## Map making

Stages are roughly:

- extract parcels inside a bbox from a .kml
- load elevation into memory from a .hgt
- get raw OSM ways from a .osm
- (elevation, raw OSM ways) -> split up OSM stuff
- find and remove disconnected things, then also compute bbox of result
- merge in the parcels fitting the specific bbox
- load traffic signal from a .shp and match to nearest intersection

- create finalish Intersection structs
- * split roads into lanes based on lane specs. also update Intersections.
- * trim road lines for each intersection
- * make turns for each intersection
- * make each building, finding the front path using lanes
- map over parcels directly

The live edits will modify lane specs and turns. Will have to re-do starred
items most likely. Should be straightforward to only redo small parts of those
stages.

## Basic geometric types

Not aiming to get it right forever, just improving the mess now.

- Pt2D
	- just a pair of f64's, representing world space (non-negative)
	- no more ordered_float; have a variant only when needed
- Angle
	- normalized, with easy reversing/perpendicularing
- Line
	- pair of points
- Polyline
- Polygon

conversions to Vec2d ONLY for graphics; maybe even scope those conversions to render/

## Lanes

It's time to model more things:

- multiple driving lanes, with possibly individual turn restrictions
- dedicated bus lanes
- lanes with parked cars
- bike lanes
- sidewalks

Each lane needs some geometry:

- center lines to draw agents on
	- for sidewalks, use center line to to draw agents on the left and right sides?
- polygons to draw the lane and mouseover

Open questions:

- Can we assume all lanes are the same width?
	- Seems wrong for many sidewalks especially
	- Could be wrong for bike lanes, but could just assume it's a bike lane with a buffer
- Some lanes are immutable
	- Sidewalks can't be changed to other types; they're raised with a curb

Some modeling questions:

- Where should expansion of roads into lanes happen?
	- initial OSM conversion, adding more stuff to the proto?
	- initial map_model::new loading, at least for development convenience
		- same reason that turns aren't (yet) serialized
- Is it useful to model the entire road?
	- the parent/child relation may be hard to maintain
	- but lanes need to know their siblings
	- maintaining directional sanity could be useful
	- what's the UI for changing lane types?
	- it's a bit arbitrary which lane should draw the yellow center lines



Initial design:
- "Road" becomes "Lane" with a type
- don't need to know sibling lanes yet
- arbitrarily, one lane might have extra bits/geometry for yellow center line markings
- ideally, get rid of one-wayness and original center points, and plumb along pre-shifted lines
	- but due to the polyline problem (affecting both geom center line layer that agents follow, and polygons for drawing), can't do this. encapsulate the messiness at least.
	- so, store one way and orig points and index, but have an accessor
	- as a compromise, dont interpet OSM points on a one-way road as the center, but as the edge? this is proving hard to do.

Thinking about a new design:
- Much more general "Land" primitive that's just a nice polygon boundary for drawing/selection and one (or more, for sidewalks?) center lines for how to cross the space, with a notion of turns. It's what road is now, but way simpler data.
- Maybe the GeomRoad / DrawRoad split is a little confusing after all, since the layering just isn't perfect. figure out the polygon and centerline up-front, then ditch the other intermediate gunk.
- also ideally make one polygon for the road, not a bunch of individual pieces? but then we'd have to go triangulate later for opengl anyway
- enforce that all the polygons are nonoverlapping

## Polylines

The polyline problem:
- https://www.codeproject.com/Articles/226569/Drawing-polylines-by-tessellation
- https://stackoverflow.com/questions/36475254/polylines-outline-construction-drawing-thick-polylines
- Will lengths change? Is this a problem?
- Drawing cars as rectangles is funky, because if their front is aligned to a new line segment, their back juts into the center of the road
- https://hal.inria.fr/hal-00907326/document
- https://www.researchgate.net/publication/220200701_High-Quality_Cartographic_Roads_on_High-Resolution_DEMs


https://wiki.openstreetmap.org/wiki/Proposed_features/Street_area

## Crosswalks

- Turns go from a src to a dst, so we'd need to double them for crosswalks, since they're always bidirectional
- Turn icons might not make sense as a UI?
- Many sidewalks directly connect at corners and shouldn't have anything drawn for them
- We don't want to draw diagonals... just from one side of the road to the other
- We want crosswalks at the beginning AND end of the sidewalk!

- v1: remember other_side for sidewalks too. draw crosswalks at the beginning AND end of every sidewalk lane.
- do extra drawing in DrawIntersection for now, figure out modeling later.

- alright, directional lanes and turns dont fit sidewalks at all. turn icons
  are drawn at one end. the turns-in-an-intersection invariant is broken, since
  src and dst dont match up for one side.
- could kind of cheat by doubling lanes for sidewalks and making the geometry
  overlap, but this feels like a worse hack. it's very tempting to ditch lanes
  and turns for a different way to model sidewalks and crosswalks.
	- for now, let's make a distinct road and lane abstraction and plumb that all the way through. see what it'd be like to have some more primitives:

- Intersection
- Building
- Parcel
- Road (undirected bundle)
- Driving/biking Lane (directed)
- Sidewalk (undirected)
- Parking lane (no orientation... or, kind of like a driving lane)
- Turn (directed and not)

but the fact that sidewalks are oriented is actually convenient, it makes it clear that incoming's last pt should be glued to outgoing's first pt.

what if we just add a bit and make turns bidirectional? still express them in the directional way?
if we're looking at turns from a road that's a sidewalk, bake in some extra logic?

## Bike lanes

How do we model bikes merging to a driving lane to make a left?

## Pedestrian modeling

- Is it useful to distinguish CarID and PedestrianID? What about when an agent has a multi-modal trip? Probably become AgentID later.

- Worth mentioning that I'm assuming pedestrians don't queue or collide. In
  most reasonable sidewalk cases, this is true. Don't need to model more
  detailed movement. As a consequence of this, crosswalk turns never conflict.
  Assume people can weave.

## Stop signs

How to depict stop signs? Each driving lane has a priority... asap go or full
stop. Turns from go lanes might be yields, but shouldn't need to represent that
visually.

- Easy representation: draw red line / stop sign in some driving lanes. Leave the priority lanes alone.
- Harder: draw a stop sign on the side of the road by some lanes. Won't this look weird top-down and at certain angles?

## Traffic signals

- per lane would be weird.
- drawing turn icons as red/yellow/green is pretty clear...
- could draw an unaligned signal box with 3 circles in the middle of the intersection, but what does it represent? maybe just an initial indicator of what's going on; not full detail.
- similarly, draw a single stop sign in the middle of other intersections? :P

## GUI refactoring thoughts

- GfxCtx members should be private. make methods for drawing rectangles and such
	- should be useful short term. dunno how this will look later with gfx-rs, but dedupes code in the meantime.
- should GfxCtx own Canvas or vice versa?
	- Canvas has persistent state, GfxCtx is ephemeral every draw cycle
	- dont want to draw outside of render, but may want to readjust camera
	- compromise is maybe storing the last known window size in canvas, so we dont have to keep plumbing it between frames anyway.


One UI plugin at a time:
- What can plugins do?
	- (rarely) contribute OSD lines (in some order)
	- (rarely) do custom drawing (in some order)
	- event handling
		- mutate themselves or consume+return?
		- indicate if the plugin was active and did stuff?
- just quit after handling each plugin? and do panning / some selection stuff earlier
- alright, atfer the current cleanup with short-circuiting... express as a more abstract monadish thing? or since there are side effects sometimes and inconsistent arguments and such, maybe not?
	- consistently mutate a plugin or return a copy
	- the Optionals might be annoying.

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

## Representing map edits

Two reasons for edits:
- the basemap is wrong because of bad OSM data or heuristics
- here's a possible edit to A/B test

Types of edits:
- change lane type between driving, parking, biking
	- sidewalks are fixed!
	- some edits are illegal... parking lane has to be in a certain side... right? well, actually, dont do that yet.
- delete a lane (because the basemap is wrong)
- modify stop sign priorities
- modify traffic signal timings

How to visually diff edits?
- highlight them
- UI to quickly jump and see them

How to encode the edits?
- "Remove lane" is weird; how about per road, list the lane types? Then it's
  almost kinda obvious how to plug into part of the current map making
pipeline.
- alright, let's really first think about road vs lane

Need to work through some edits to see how they affect downstream things. What
needs to be recomputed? How do we long-term serialize things like edits? How
can they even refer to things by ID if the IDs could change? What IDs might
change?

Alright, now we can be concrete -- when we have a road edit, what can be affected?

MAP LAYER:

- the road struct state (just list of children, really)
	- dont want to blindly run all the road making code, since it'd double-add stuff to intersection
- delete old lanes, make new lanes
	- how would IDs work? if we try to reuse the old ones, we might wind up
	  with gaps, or overflowing available space.
- trim lanes
	- need to recalculate original lane_center_pts for all affected lanes
	  in a certain direction. tricky since they're two-sided; have to
	  restore just the original direction on it.
- recalculate turns, for the two intersections
	- same ID problem
- recalculate some building front paths, maybe

CONTROL LAYER:

- recalculate two intersections

SIM LAYER:

- creating/deleting sidewalks is pretty easy
- SimQueues are associated with turns and lanes, but easyish to create/delete later
- should probably have a way to prevent mutations; maybe need to drain a lane of agents before changing it

UI:

- make a new DrawLane, DrawIntersection, etc
- update quadtrees
- would have to maybe update a bunch of plugin state (highlighting or
  floodfilling or something), but since we know road editor is active, is easy!



Strategies:
- testing via equivalence -- reload from scratch should be equal to live edits
	- will IDs make this very tricky?
- for things like sim and UI that hook on and have derived state, should we
  always kinda lazily grab DrawRoads, SimQueues, etc? or immediately plumb
  through deletes and inserts?
- is there a way to programatically record data dependencies or kinda do FRPish stuff from the start?
- could always blindly recalculate everything live, but man, that's gotta be slow
- maybe change constructors that take full map into incremental "hey, this road exists!" mutations. then just need to introduce deletions. in other words, embrace incremental mutability.
- assume the bbox doesn't change as a result of any edit



the ID problem:
- need determinism and deep equality checks for things. if we load a map from
  scratch with edits, vs do a live shuffle, the IDs wont match up if they use a
  slotmap.
- can we refer to things in more stable ways; no LaneID, but
  RoadID+direction+offset. no Turn, but two... effectively lane IDs?
- maybe we can combine these ideas; use nondet slotmaps, but when doing
  equality checks, dont use these IDs -- treat these IDs as memory addresses.
  IDs for lookup and IDs for equality.
- what're the different things that need this?
	- stable objects: building, intersection, parcel, road
	- malleable
		- lane (road, direction, offset, lane type)
		- turn (src lane, dst lane)
			- recurse and refer to full lane descriptions, or their temporary ID?
- ideally want to store things contiguously in memory
- ideally want a compact, easy thing to type quickly to debug.
- aka, ideally want a nice bijection from the persistent thing to numbers?
- actually, if we "leave room for" enough lanes per road and turns per intersection to begin with...
	- can just replace existing IDs when we change something
	- still have to mark things dead
	- still have to watch out for dangling references


The changes needed:
- figure out the ID problem
- change existing code from big constructors to incremental adds
	- exactly what layers and objects?
- implement incremental deletes
- try doing a live edit and comparing with from scratch


Going to start implementing part of this in a branch, just to get more detail.

- when there's a road edit, calculate the affected objects (road and all children, two intersections)
- implement a sanity check to make sure no dangling ref to old IDs

I think this is working so far. The vital question: is it too complicated? Is there a simpler way?
- simpler idea: retain more raw data, violently destroy road and intersection and make from scratch
	- problem: it'd percolate, we need to keep old connected roads the same
- EVEN SIMPLER IDEA: stop trying to solve hard problems
	- lane deletion is rare and a basemap-only edit; can mark it in the UI temporarily and omit in the next full load
	- changing lane types is the main intended edit. what actual consequences does this have? filtering through the list earlier...
		- change lane type
		- recalculate all turns for two intersections
			- the number of turns might go up or down
		- control layer intersection policies then need updating
		- sim needs to know about changed lanes and turns
		- and a few easy edits in the UI layer too
	- changing lane direction might be a little more complicated, but NOT BY MUCH

so, I think the steps:
= see what's useful from this branch, bring it to master (encapsulating the driving state stuff)
= ditch TurnIDs; have a BTreeMap of src/dst (LaneID, LaneID)
= add a mutate_lanes() and replace_turns() to all the appropriate layers

Cool, good enough to start. whew.

## Spawning agents ##

Ideally, each method would return a future that would do stuff, right? But the
mutations need to be done serially. So I think each sim's method should take
the path outright, not even start/end. Stick the rng work in sim for the
moment. This should let the start/goal selection and the parallelization of
paths happen at a more outer layer, in the sim aggregator.

... and now for scenarios / spawners. these get to run every step, trying to
introduce new things in the different simulations. if a parked car can't
currently begin departing, it'll keep trying every tick.

## Intersection policies for pedestrians ##

Before figuring out how pedestrians will deterministically use intersections alongside cars, recall how cars currently work...

- ask all cars for next move (continue on same thing, or move to a turn/lane)
- using fixed state, adjust some of the moves that dont have room to move to a new spot to wait instead
- serially ask intersections if a car can start a turn
- serially make sure only one new car enters a lane in the tick
	- shouldnt the intersection policy guarantee this by itself?
- very awkwardly reset all queues from scratch

How did AORTA do it?

- agent.step for all of em (mutate stuff)
	- enter intersections, telling them. must've previously gotten a ticket
- let all the agents react to the new world (immutable, except for IDEMPOTENTLY asking for turn)
	- here we ask for tickets, unless we've already got one
- same for intersections
	- grant them here

aka basically yeah, the simple:

- agents send a ticket during the planning phase?
- intersections get a chance to react every tick, granting tickets
- during the next action phase, an agent can act on the approved ticket?

good pattern in intersections:
- a sim state that the rest of the code interacts with for ALL intersections. rest of code doesnt see individual objects.
- that manager object delegates out most of the logic to SPECIALIZED versions of individual objects and does the matching
	- no need for this to exist on the individual IntersectionPolicy object

How to share common state in intersections?
- if it's just the accepted set, have a parallel array and pass it into step()
	- data locality gets ruined, this is ECS style, bleh
- have a common struct that both enum variants contain
	- still have to match on enum type to operate on it commonly!
- have one struct that then contains an enum
	- when delegating to specialized thing, can pass this unpacked thing down, right?

Seeing lots of deadlock bugs from accepting non-leader vehicles. For now,
switch to only considering leader vehicles, and later maybe relax to anybody
following only accepted vehicles.

Leader vehicle is a bit vague; could be leader on current queue, which is still a bit far away.

## Notes on determinism ##

- serde tricks
- unit tests

## AORTA driving model ##

- figure out how to dynamically sub out two different driving models. traits or generics on Sim.
	- repeat determinism tests for both!
- start sharing code between the two models (and the ped model)

the polymorphism mess:
	- a sim can contain one of two different driving models
	- driving models both implement the same interface (a trait)
	- need to serialize/deserialize them, but cant get deserialization with erased_serde working
	- so use enums and just delegate everything. macro?
	- tried deref on an enum, returning a trait
	- delegate crate can't handle match expressions

## Floating point and units ##

Currently using si::Second<f64> for time, which means comparing sim state by
deriving Eq is a headache. Since the timestep size is fixed anyway, this should
just become ticks. Was tempted to use usize, but arch dependence is weird, and
with a 0.1s timestep, 2^32 - 1 ticks is about 13.5 years, which is quite a long
timescale for a traffic simulation. :) So, let's switch to u32.

Now I'm hitting all the fun FP imprecision issues. Could probably hack in
epsilon and negative checks everywhere in kinematics, but why? Should research
it more, but I think the better approach is to use fixed-point arithmetic for
everything (aka u8 or whatever).

- moment in time (tick)
	- resolution: 0.1s with u32, so about 13.5 years
- duration
	- resolution: 0.1s with u32, so about 13.5 years
	- time - time = duration
- distance (always an interval -- dist_along is relative to the start)
	- non-negative by construction!
	- say resolution is 0.3m (about 1ft), use u32, huge huge distances
	- geometry is polylines, sequences of (f64, f64) representing meters
	  from some origin. we could keep drawing the same, but do stuff like
	  dist_along as this new type? or change Pt2D to have more reasonable resolution?
	- still represent angles as f64 radians? for drawing, turn calculation, etc
- speed
	- meters per second, constructed from distance / duration
	- should resolution be 0.3m / 0.1s? 1 unit of distance per one timestep? or, maybe less than that?
	- can be negative
- acceleration
	- can be negative
	- what should the resolution be?

## Simulation unit tests

To encourage testing, it should be easy to:
	- describe a setup
	- assert what the outcome should be
		- sometimes just that no runtime invariants are broken
	- pop up a UI to interactively step through the test

Some first tests to write:
	= car starting with no path on road with no parking spots, ensure they wind up parking at the first spot on some
	side street            
	- car stops for departing car (winds up following it)
	- departing car waits for other car (winds up following it)
	- a line of cars moving through a stop sign looks jittery right now. correct or not?
	- following distances for cars of different lengths

Unclear how to nicely let the test inspect stuff every tick.

Rejected ideas:
- make every pub(crate) so a unit test can reach into state anywhere. It ruins viz for testing.
- JSONify stuff and look at that. too slow, and walking the JSON structure is annoying and not type-safe.
- make one-off accessors for interesting stuff. pollutes code and is tedious.

The idea that's sticking:
- every tick, accumulate a list of events that occurred. publish these from various places.
	- most of the events are state transitions -- car leaves lane, intersection accepts ticket, car parks, bus departs
	- beyond unit testing, this will be useful for building up a compressed schedule for the time traveler
	- and already am kind of using this pattern to communicate between sim managers, spawners, etc
	- will help compute trip statistics later
	- would also be nice to log some of these

## Per-car properties

Need to associate car length between driving and parking sims.

---> could store this in master Sim; after all, there will be some more permanentish stuff like agent/building/trip/owned car soon
	- but soon need to bundle things together and pass less params everywhere
- or stash in parking sim, transfer to driving, and back later

Wait, length of car affects parking pretty critically. A bunch of things plumb
around the precomputed front of the spot, used for drawing and for cars to line
up their front in the sim. I think we need to plumb the true start of the spot
and have a method to interpolate and pick the true front.

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

## Notes on King County GIS datasets

- TODO: https://data-seattlecitygis.opendata.arcgis.com/datasets/channelization

- https://data-seattlecitygis.opendata.arcgis.com/datasets/street-signs

## Stop sign priority

Use OSM highway tags to rank. For all the turns on the higher priority road, detect priority/yield based on turn angle, I guess.

## Watch tests easily

- need to organize savestate captures
	- dedicated place: data/savestates/MAP/scenario/time
		- plumb map name, scenario name
		- should be able to just point to one of these saves, not refer to the map or RNG seed again
	- also kinda needed for time traveling later

- when a problem happens, we want to back up a little bit
	- probably just need automatic occasional savestating, and to print a nice command to rerun from it

## Diffing for A/B tests

Basic problem: how do we show map edits/diffs?
	- could be useful for debugging as new data sources come in
	- and is vital part of the game
	- UI
		- highlight edited things
		- hold a button to show the original versions of things in a transparentish overlay

How to show diffs for agents?

## Bus

Before even looking at the GTFS schema, how should we model buses? They're
basically a special car that just goes from bus stop to bus stop (a
SidewalkSpot) and then just stops for a little while. The route is fixed, but
for now, since pathfinding ignores live traffic, probably fine to ignore this.

- step 1: hardcode two BusStops and hardcode spawn a car that cycles between the two
	- render a bus stop on the sidewalk
		- this actually belongs to the map layer! associated with a sidewalk I guess.
	- render the bus in a special color, and also, make it really long (adjust following dist, but not parking spot len)
	- how to unit test that a bus has reached a stop and is waiting? how do we even know that a bus is at a stop for peds to soon board it? I think a transit state will happen soon...

- step 2: make some peds pick a SINGLE bus to use for their route, if it helps

- step 3: make peds load on the bus and get off at the correct stop. make buses usually wait a fixed time at each stop, but wait a littl extra if loading passengers takes a while.
	- should walking state own peds waiting for a bus?
		- yes: easier drawing, later need to know how crowded a sidewalk is, it's just weird to keep indicating we're at a place. router for cars does this, and the transit sim holds the higher-level state. do the same for now.
			- kind of want walking sim to have a list of peds idling at bus stops. transit sim can let all of them know when a bus arrives!
		- no: transit sim can also contribute DrawPeds. the walking layer has nothing left to do with them... right?

		so:
		1) ped reaches bus stop, writes event. walking sim flips a bit so they stop trying to step(). also has a multimap of bus stop -> waiting peds. they continue to exist on their sidewalk for rendering / crowding purposes.
		2) transit sim gets a step(). for every bus that's waiting, it queries walking sim to see what peds are there. ??? trip thingy will decide if the ped takes the bus or not, but the ownership transfer of ped from walking to transit happens then.
		3) when a bus initially arrives at a stop, it queries all passengers to see who wants to get off and join the walking sim again. the trip thingy decides that.

- step N: load in GTFS for seattle to get real routes and stops

later: multiple transfers, dedicated bus lanes, light rail...

Actually, jump to step 3 and just hardcode a ped to use a route, for now. what should the setup be? hardcode what stop to go to, what route to use, what stop to get off at? trip plan is a sequence...

- walk to a sidewalk POI (bldg, parking spot, bus stop)
- drive somewhere and park
- ride bus route until some stop

for now, these trip sequences can be hardcoded, and planned later.

What's the point of the spawner? It does a few things, and that feels messy:
- vaguely specify a scenario later, with things happening over time.
	- except this is unimplemented, and is probably easier to understand as a list of trips with start times
- a way to retry parked->driving car, since it might not have room
- a way to parallelize pathfinding for the ticks that happen to have lots of things spawning
- a way to initially introduce stuff
	- asap things like a bus route and parked cars
	- indirect orders, like make some parked car start driving creates a trip to satisfy that
- handling transitions to start the next leg of a trip
	- this is the part I want to split out! it's very separate from the rest.


step 1: move existing trip stuff to its own spot, but owned by spawner still
step 2: move interactive and testish spawning stuff to init() or similar, leaving spawner as just mechanics and transitions
	- spawner shouldnt have rng, right?
	- sim needs to hand out its internals (spawner, each model) for the spawning
		- separate methods that take sim and call a special method to get direct access to things?
		- i just physically want the code in a separate file. can we implement a trait in a second file?
step 3: enhance the trip stuff to have a concept of hardcoded legs, and make it choose how to use a bus
	- seed a trip using a bus
	- wire up the transitions to board and deboard the bus
	- test a basic bus scenario


## Everything as FSMs

Driving and walking layer are both kind of broken, since they know about
parking spots and bus stops. Feels like they need to be dumb, mechanical layers
that're guided by higher-level behaviors, which understand trips and such.

Could be simpler to flatten. Call each sim and dont affect other sims, then
process all events last to do transitions. except maybe one sim expects the
transition to happen immediately, so have to process events between each one?

Just to kind of document the dependency/call-chain right now...

- sim step
	- TODO spawner step
	- driving step
		- router
			- transit.get_action_when_stopped_at_end
				- this changes bus state (wouldnt it be nicer to 
		- foreach parked car as a result, add to parking sim and tell spawner that car reached spot


ask: a way to statically or at runtime print the call-chain, stopping at std libs?
	- whenever we push onto events, dump stack?

for each api method in each sim, manually look at its call chain

- transit
	- create_empty_route, get_route_starts, bus_created
		- spawn.seed_bus_route
			- sim.seed_bus_route
	- get_action_when_stopped_at_end (changes bus state, returns new path), get_dist_to_stop_at
		- router.react_before_lookahead
			- driving per car react
				- driving step
	- step (lots of state transitions happen here, but call stack is simple!)
- walking
	- ped_joined_bus
		- transit.step

A good measue of how well-structured the code is: how hard would it be to add a
few more delays / states, like time after parking the car (out of the way of
the driving sim) but before getting out of the car?

## Routers, lookahead, cars as FSMs

Hmm, hard to figure out the interactions for the router. when should it get a
chance to roam around for parking? should lookahead make copies of it (harder
to then roam...) or should it index into one of them (preferred...)


rethink cars as a much more careful FSM. on a lane, on a turn, on the LAST lane
lining up for something, parking, unparking, idling at a bus stop. these states
imply things like position (which queue to occupy space in). both lookahead and
physically stepping forwards ought to be able to use the same code -- the
routing, turn<->lane transitions, leftover dist calculation, etc shouldnt care
if its real or projected


Try this again, but with a much more careful interface for the router. But when do we get to mutate?

- during react() when we up-front see the path is empty and theres no parking
	- problem: what if lookahead earlier spots this case and doesnt know where to go?
	- soln: who cares. when that happens, just recommend stopping at the
	  end of the lane. when we physically get there, this case will trigger
          and we can mutate.
- during react() lookahead when we first try to go beyond the end
	- problem: but then it's hard to actually do the lookahead; we cant
	  clone the router and transition it along during lookahead. we would
	  have to go update the original one somehow.
- in react() lookahead, return an indicator and then have step() call another mutable method later.
	- this is the confusing kind of split I'm trying to avoid.

So I think the first solution works best.


Urgh, we have a mutability mess again -- react() needs a mutable router, but
immutable view into world state to query sim queues and find next car in front
of. Similar to needing to know speed and leader vehicles for intersection sim.
For now, should we do the same hack of copying things around?
	- kinda awkward that routers and cars are separate; a router belongs to
	  a car. maintaining the mappings in the same spots is gross. need to
          express in react() that only one part of a car -- the router/plan -- is
          mutable.
	- it's also horrible that DrivingSimState is passed around to SimQueue to get car positions!
		- an alt is to duplicate state into the simqueue and store the dist along. this would be pretty easy actually...
	- and passing around properties / all the vehicles is awkward
	- maybe it's time to cave and try an ECS?
		- just start by listing out what needs to happen.
		- car react needs mutable router, immutable queues of lanes and turns, immutable dist/speed of other cars
		- intersections need to know car speed, whether cars are leaders (so simqueue positions)

Short term solution: copy a huge nasty thing and pass it into react(). :\
	- but I'm still not convinced any mutabulity in react() is good

## Modeling choices

Don't model sidewalk crowdedness or bike rack availability, because in
practice, these things are never scarce resources or problematic. Finding a
parking spot is difficult and impacts the quality of one trip and causes
externality, so we should model that.
