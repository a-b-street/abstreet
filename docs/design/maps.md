# Map-related design notes

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

## Notes on King County GIS datasets

- TODO: https://data-seattlecitygis.opendata.arcgis.com/datasets/channelization

- https://data-seattlecitygis.opendata.arcgis.com/datasets/street-signs

## Speeding up map loading

- can we use quadtrees for the expensive building/sidewalk matching?
	- awkward to bring a rendering-layer concept in; dont otherwise care about lane polygons

## Neighborhoods

It's hard to zoom out and quickly pinpoint where interesting things (A/B diffs,
traffic jams, suspicious silence) are happening. What if we could optionally
collapse a region into a big colored polygon and just display quick stats on
it?

- defining them
	- what do they include?
		- a polygon capturing buildings, lanes, etc
		- what happens when the polygon only partially contains an object?
			- is it a border thing? makes sense for lanes, not for buildings
			- border lanes could be used for some kind of in/out flow
	- do they have to fully partition the map?
		- they should at least be disjoint
	- how to define them?
		- the seattle neighborhood definitions are seemingly way too large
		- could manually draw them, but including buildings carefully is sort of hard
		- we have the automatic parcel grouping stuff for coloring... could it help?
		- could find max-cuts to spot important border roads and the neighborhoods they connect
	- summary stats
		- cars parked, open parking spots, moving peds, moving cars,
		  stuck cars, busses present, number of agents with A/B test
                  divergence...
		- this can start to force me to be mildly performant.
		  precompute what objects are in each polygon, then have a
		  summary thing collect stats every few seconds when shown?
	- do these need to be objects in the GUI? at first no, just make a
	  plugin draw them, but eventually, yes. they should probably be a
	  map_model concept.

## Invariants

I thought I had a list of these somewhere else?

- min length for lanes, turns
- length for related lanes (sidewalk spot / parking / driving) matching up
- connectivity
- no loop lanes (same src and dst endpt)... but what about cul-de-sacs then?

## Border nodes

No matter how things are sliced, we always have to cut off roads somewhere,
unless we simulate a continent. :) So the idea of border nodes is to
start/terminate these cut-off roads with a special intersection where traffic
can begin or end.

- rendering?
	- ideally the nodes would actually be at the boundary of the map
		- https://wiki.openstreetmap.org/wiki/Osmosis/Detailed_Usage_0.47#Area_Filtering_Tasks
		  has some flags to explore
	- some special color or symbol?
- detection
	- for oneways this is easy, but two-ways look like dead-ends
	- how to distinguish actual dead-ends?
	- get osmosis to also output which OSM ways were clipped?
		- don't see how to do this
	- manually marking them?
- draw the FSM for cars/peds
	- trips starting/ending at border nodes short-circuit some steps

What about border nodes where the sidewalk can cross over, but the driving lane
leads nowhere? Maybe instead of having a special intersection policy, we have
turns to nowhere? Sort of depends how the border node is defined -- do we have
long skinny things jutting out, or not?
	- don't add any turns, even if we could

OK, now adapt the control and sim layers to handle border nodes...

Adapt trips / pathfinding. It has to be an explicit origin/goal.

start/end for trips right now is buildings. Needs to be building or a border
node. And driving also gets more complicated -- can start from a parked car or
a border node, and end by parking near a building or going to a border node.

### The master FSM

This doesn't belong in maps.md, but related to border nodes, so for now...

There's ultimately a big state machine for trips that's awkwardly hiding in the
code and slowly being exposed by stuff like the higher-detail pathfinding.

- possible starts:
	- ped exits building
	- ped appears at border node
	- car appears at border node

- possible ends:
	- ped enters building
	- ped vanishes at border node
	- car vanishes at border node

- the stuff in the middle
	- ped crosses front path from building to sidewalk spot
	- ped crosses front path from sidewalk spot to building
	- agent crosses a lane normally
	- agent crosses a lane contraflow
	- agent makes a turn
	- unparking a car from a spot
	- parking a car at a spot
	- (soon) lanechanging
	- wait for a bus
	- enter a bus
	- ride a bus
	- exit a bus

- Roaming around looking for parking dynamically and lazily updates the front of this plan

- Buses are weird
	- it's annoying they don't have a trip (and don't work with the route plugin today)
	- they share some states (the mechanical driving ones)
	- and have a few of their own (deboard people, board people)
	- dynamic plan expansion also happens when departing from a stop; since
	  storing the entire cycle would be weird
- Parked cars aren't really part of this giant state machine

Then the code for these little transitional states can be less weirdly special cased, maybe.

How does initial plan formation and mode choice work? Could have a round of
pathfinding for each possible mode, or could search a single more abstract
action graph. moves from walking on this sidewalk? oh we own this car, could
choose to unpark it, then drive somewhere. Everything has time cost.

This also supercedes the Event thing in sim and makes testing potentially WAY cooler.

## Pathfinding

How do we natively wind up with a list of PathSteps (normal lane, contraflow
lane, turn) without doing the terrible stitch-together later thing?

- easy option: the nodes in the graph we search become PathSteps. expansion is easy.
	- is it slightly inefficient that we could loop back and forth on
	  sidewalks? Should be able to prevent it by not making that a way to
          expand.
- maybe less weird option: nodes are (Lane, Intersection). or even more
  accuratelyish, (Lane, dist_along), so we can start and end anywhere.
	- When stitching together the path, a pair of lanes is a turn.
	  Otherwise, cross forwards or backwards.

## Road query refactor

What we have right now...

Road
    pub fn find_sidewalk(&self, parking_or_driving: LaneID) -> Result<LaneID, Error>
    pub fn find_driving_lane(&self, parking: LaneID) -> Result<LaneID, Error>
    pub fn find_driving_lane_from_sidewalk(&self, sidewalk: LaneID) -> Result<LaneID, Error>
    pub fn find_parking_lane(&self, driving: LaneID) -> Result<LaneID, Error>

    pub fn get_opposite_lane(&self, lane: LaneID, lane_type: LaneType) -> Result<LaneID, Error>
    pub fn get_siblings(&self, lane: LaneID) -> Vec<(LaneID, LaneType)>

Map
    pub fn get_driving_lane_from_bldg(&self, bldg: BuildingID) -> Result<LaneID, Error>
    pub fn get_driving_lane_from_sidewalk(&self, sidewalk: LaneID) -> Result<LaneID, Error>
    pub fn get_sidewalk_from_driving_lane(&self, driving: LaneID) -> Result<LaneID, Error>
    pub fn get_driving_lane_from_parking(&self, parking: LaneID) -> Result<LaneID, Error>

    pub fn building_to_road(&self, id: BuildingID) -> &Road

Some themes...

- have some lane, want the nearest lane of some type(s).
- The source doesn't really matter -- we want as close as possible, but there's no requirement of adjacency at all.
- If the source/dest is a sidewalk, some weird handling for one-ways...
