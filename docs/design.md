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

## Sim state equality and f64's

Currently using si::Second<f64> for time, which means comparing sim state by
deriving Eq is a headache. Since the timestep size is fixed anyway, this should
just become ticks. Was tempted to use usize, but arch dependence is weird, and
with a 0.1s timestep, 2^32 - 1 ticks is about 13.5 years, which is quite a long
timescale for a traffic simulation. :) So, let's switch to u32.

## UI plugins

- Things like steepness visualizer used to just be UI-less logic, making it
  easy to understand and test. Maybe the sim_ctrl pattern is nicer? A light
adapter to control the thing from the UI? ezgui's textbox and menu are similar
-- no rendering, some input handling.

## Map making

Stages are roughly:

- extract parcels inside a bbox from a .kml
- load elevation into memory from a .hgt
- get raw OSM ways and bbox from a .osm
- (elevation, raw OSM ways) -> split up OSM stuff
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
