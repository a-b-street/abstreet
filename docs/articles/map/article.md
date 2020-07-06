# A/B Street's map model

This article describes how data from OpenStreetMap (OSM) and King County GIS
become the complex maps in A/B Street. As always, email <dabreegster@gmail.com>
if you'd like more details or pictures. This process generalizes to most cities
in OpenStreetMap. Some extra data specific to Seattle is used, but could be
omitted.

TODO: Integrate pictures from
[these slides](https://docs.google.com/presentation/d/1cF7qFtjAzkXL_r62CjxBvgQnLvuQ9I2WTE2iX_5tMCY/edit?usp=sharing).

[This recorded presentation](https://youtu.be/chYd5I-5oyc?t=439) covers some of
this.

Everything here should be up-to-date as of June 2020.

<!--ts-->
   * [A/B Street's map model](#ab-streets-map-model)
      * [The final map](#the-final-map)
         * [Coordinate system](#coordinate-system)
         * [Invariants](#invariants)
      * [From OSM to RawMap (convert_osm crate)](#from-osm-to-rawmap-convert_osm-crate)
      * [RawMap to InitialMap](#rawmap-to-initialmap)
      * [InitialMap to Map](#initialmap-to-map)
      * [Live edits](#live-edits)
      * [Development tricks](#development-tricks)
      * [Appendix: PolyLines](#appendix-polylines)

<!-- Added by: dabreegster, at: Sun Jun 21 16:17:03 PDT 2020 -->

<!--te-->

## The final map

A/B Street comes with a few maps, each defined by a bounding/clipping polygon
for some portion of Seattle. Each map has these objects:

- **Roads**: A single road connects two intersections, carrying OSM metadata and
  containing some child lanes.
- **Lanes**: An individual lane of traffic. Driving (any vehicle), bus-only, and
  bike-only lanes have a direction. On-street parking lanes don't allow any
  movement, and they have some number of parking spots. Sidewalks are
  bidirectional.
- **Intersections**: An intersection has references to all of the incoming and
  outgoing lanes. Most intersections have a stop sign or traffic signal policy
  controlling movement through it.
  - **Border** intersections on the edge of the map are special places where
    agents may appear or disappear.
- **Turns**: A turn connects one lane to another, via some intersection.
  (Sidewalks are bidirectional, so specifying the intersection is necessary to
  distinguish crosswalks at each end of a sidewalk.)
- **Buildings**: A building has a position, OSM metadata, and a **front path**
  connecting the edge of the building to the nearest sidewalk. Most trips in A/B
  Street begin and end at buildings. Some buildings also contain a number of
  off-street parking spots.
- **Area**: An area has geometry and OSM metadata and represents a body of
  water, forest, park, etc. They're just used for drawing.
- **Bus stop**: A bus stop is placed some distance along a sidewalk, with a
  pointer to the position on the adjacent driving or bus lane where a bus stops
  for pick-up.
- **Bus route**: A bus route has a name and a list of stops that buses will
  cycle between. In the future, they'll include information about the
  frequency/schedule of the route.
- **Parking lot**: A parking lot is connected to a road, has a shape, and has
  some internal driving "aisles." The number and position of individual parking
  spots is auto-generated.

### Coordinate system

A/B Street converts (longitude, latitude) coordinates into a simpler form.

- An (x, y) point starts with the top-left of the bounding polygon as the
  origin. Note this is screen drawing order, not a Cartesian plane (with Y
  increasing upwards) -- so angle calculations account for this.
- The (x, y) values are f64's trimmed to a few decimal places, with way more
  precision than is really needed. These might become actual fixed-point
  integers later, but for now, a `Pt2D` skirts around Rust's limits on f64's by
  guaranteeing no NaN's or infinities and thus providing the full `Eq` trait.
- A few places in map conversion compare points using different thresholds,
  usually below 1 meter. Ideally these epsilon comparisons could be eliminated
  in favor of a fixed-point integer representation, but for now, explicit
  thresholds are useful.

### Invariants

Ideally, the finalized maps would satisfy a list of invariants, simplifying the
traffic simulation and drawing code built on top. But the input data is quite
messy and for now, most of these aren't quite guaranteed to be true.

- Some minimum length for lanes and turns. Very small lanes can't be drawn, tend
  to break intersection polygons, and may lead to gridlocked traffic.
- Some guarantees that positions along adjacent lanes actually match up, even
  though different lanes on the same road may have different lengths. Examples
  include the position of a bus stop on the sidewalk and bus lane matching up.
  - Additionally, parking lanes without an adjacent driving lane or bus stops
    without any driving or bus lanes make no sense and should never occur.
- Connectivity -- any sidewalk should be reachable from any other, and most
  driving lanes should be accessible from any others. There are exceptions due
  to border intersections -- if a car spawns on a highway along the border of
  the map, it may be forced to disappear on the opposite border of the map, if
  the highway happens to not have any exits within the map boundary.

## From OSM to RawMap (`convert_osm` crate)

The first phase of map building reads in data from OSM files and a few others,
producing a serialized `RawMap`. Importing all maps (one for each pre-defined
bounding polygon) takes a few minutes. Players don't see this cost; it only
takes a few seconds to load a serialized map.

- `osm.rs`: Read .osm, extracting the points for road-like ways, buildings, and
  areas
  - Areas usually come from a relation of multiple ways, with the points out of
    order. Gluing all the points together fails when the .osm has some ways
    clipped out. In that case, try to trace along the map boundary if the
    partial area intersects the boundary in a clear way. Otherwise, just use a
    straight line to try to close off the polygon.
  - Also read traffic signal locations and turn restrictions between OSM ways
- `split_ways.rs`: Split OSM ways into road segments
  - OSM ways cross many intersections, so treat points with multiple ways and
    the points at the beginning and end of a way as intersections, then split
    the way into road segments between two intersections.
  - This phase remembers which road segment is the beginning and end of the OSM
    way, for per-lane turn restrictions later
  - Apply turn restrictions between roads here. Since OSM ways cross many
    intersections, the turn restrictions only apply to one particular road
    segment that gets created from the way. Make sure the destination of the
    restriction is actually incident to a particular source road.
- `clip.rs`: Clip the map to the boundary polygon
  - Osmosis options in `import.sh` preserve ways that cross the boundary
  - Trim roads that cross the boundary. There may be cases where a road dips out
    of bounds, then immediately comes back in. Disconnecting it isn't ideal, but
    it's better to manually tune the boundary polygon when this happens than try
    to preserve lots of out-of-bounds geometry.
  - Area polygons are intersected with the boundary polygon using the `clipping`
    crate
- `lib.rs`: Remove cul-de-sacs (roads that begin and end at the same
  intersection), because they mess up parking hints and pathfinding.
- `lib.rs`: Apply parking hints from a King County GIS blockface dataset
  - Match each blockface to the nearest edge of a road
  - Interpret the metadata to assign on-street parking there or not
- `lib.rs`: Apply offstreet parking hints from a King County GIS dataset
  - Match each point to the building containing it, plumbing through the number
    of spots
- `lib.rs`: **Disabled**: Apply sidewalk presence hints from a King County GIS
  dataset
  - Match each sidewalk line to the nearest edge of a road
  - Update the road to have a sidewalk on none, one, or both sides
- `lib.rs` using the `srtm` module: Load (extremely poor quality) elevation data

## RawMap to InitialMap

The remainder of map construction is done in the `map_model` crate. There's one
intermediate structure between `RawMap` and `Map`, called `InitialMap`.

- `make/remove_disconnected.rs`: Remove disconnected roads
  - Just floodfill from some road, assuming all roads are bidirectional, to get
    different partitions.
  - Remove roads from all but the largest partition
- `make/initial/mod.rs` and `make/initial/lane_specs.rs`: Interpret OSM tags to
  figure out what lanes are on each side of each road, also figuring out the
  total width of the road.
- `make/initial/geometry.rs`: Figure out the polygon for each intersection, and
  trim back road center-lines to end at a face of the polygon.
  - For every road touching the intersection, get the polyline of each side,
    based on the road's width
    - See appendix for how to shift polylines
  - Sort all the polylines by the angle to the intersection's shared point
  - Intersect every polyline with every other polyline
    - More specifically -- the second half of each polyline, to get the correct
      collision point
    - Look at the perpendicular infinite line to the collision point on the
      shifted polyline, then find where it hits the original center line. Trim
      back the center line by the max distance from these collisions.
  - Compute the intersection's polygon by considering collisions between
    adjacent roads' polylines
  - Deal with short roads and floating point issues by deduping any adjacent
    points closer than 0.1m

## InitialMap to Map

Still in the `map_model` crate.

- `map.rs`'s `make_half_map`: Expand roads to lanes, using the list of lane
  types from before
- `make/turns.rs`: Generate turns for every intersection.
  - Vehicle turns (for cars, bikes, buses)
    - Consider every pair of roads in the intersection. Try to match up lane
      types -- if there's a bike lane on both roads, don't add a turn from
      driving->bike or bike->driving. If there's not, then fallback to
      transitions between different lane types.
    - Classify the turn based on the difference between the angle of the
      incoming lane's last line and the outgoing lane's first line
      - For straight turns, use the Cartesian product to link every incoming
        with every outgoing lane. If the indices dont match up, the turn becomes
        a `LaneChangeLeft` or `LaneChangeRight` turn. This is used later for
        intersection policies to prioritize turns appropriately.
      - Right and left turns only originate from the one lane on the appropriate
        side
  - Walking turns for pedestrians
    - Consider pairs of adjacent roads around the intersection
      - Make a crosswalk to the other side of the road, assuming there's a
        sidewalk on both sides
      - Make a shared sidewalk corner over to the adjacent road
      - If the adjacent road doesn't have a sidewalk on the close side, then
        consider skipping that road and making a crosswalk over to the next
        road. An example of this is a crosswalk over a highway on/off ramp.
  - Verify all the turns so far are unique
  - Filter by the OSM turn restrictions ("only straight" between road1 and
    road2)
  - Try to apply the OSM per-lane restrictions ("straight or left" from lane 3)
    - The number of lanes in the OSM metadata might not match up with how many
      lanes created
    - Some of these OSM tags are just completely wrong sometimes. If the filter
      makes an incoming lane lose all of its turns, then ignore that tag.
- `make/parking_blackholes.rs`: Find well-connected roads near "blackhole"
  lanes.
  - Starting from most driving/biking lanes, most other lanes are reachable.
    Some aren't -- such as one-way highways inevitably leading from or to a
    border. These are "blackholes" -- pathfinding to or from here may fail.
  - Find the largest strongly-connected component (SCC) in the driving graph.
    From every other lane (a blackhole), floodfill both forwards and backwards
    to find the nearest driving lane part of the main SCC.
  - Later, if a car needs to park by a building on a blackhole road, it'll
    instead start searching for parking at the redirect. This prevents it from
    being forced to instead exit the map through a border.
- `make/buildings.rs`: Match buildings up with sidewalks
  - Find the closest sidewalk polyline to each building's center. Then draw a
    straight line for the front path between the edge of the building and the
    sidewalk point.
  - Filter out buildings too far away from any sidewalk
  - The front path might cross through other buildings; this is probably not
    worth fixing.
- `make/buildings.rs`: Same for parking lots
  - Similar process to match parking lots to nearest sidewalk and driving lane
  - Try to place parking spots along both sides of parking aisles
  - Filter out overlapping spots
- `make/bridges.rs`: Find what roads lie beneath bridges, and update their
  Z-order accordingly for later drawing.
- `stop_signs.rs`: Instantiate default stop sign policies
  - Rank incoming roads by OSM priority (arterial beats residential)
  - If there's only one rank, then make an all-way stop
  - Otherwise, the highest rank gets priority and others stop
    - Check if there are any conflicts based on this. If so, then fall-back to
      an all way stop.
- `traffic_signals.rs`: Instantiate default traffic signal policies
  - Apply the first predefined policy that works.
    - 4-way 4 phase, 4-way 2 phase, 3-way 3-phase, degenerate policy for 2
      roads, 2-phase for 4 one-ways
    - Fallback to a greedy assignment that just randomly starts a new phase,
      adds all compatible turns, and repeats until all turns are present
      priority in some phase.
- `pathfind/mod.rs`: Prepare pathfinding
  - A/B Street uses contraction hierarchies (CH) for fast routing, using the
    `fast_paths` crate.
  - `pathfind/vehicle.rs`: For cars, bikes, buses
    - There's a separate CH for cars, buses, and bikes, since they can use
      slightly different sets of lanes.
    - Building the CH for buses and bikes is much faster than the one for cars,
      because the algorithm can re-use the node ordering from the first CH.
    - Every lane is a node in the graph, even if it's not an appropriate lane
      type -- it might change later, and reusing orderings is vital for speed.
    - If two lanes are connected by a turn, then there's an edge in the graph.
      - The edge weight is the length of the lane and turn. Later this could
        take into account speed limit, penalize lane-changing and left turns,
        etc.
  - `pathfind/walking.rs`: For pedestrians
    - Only sidewalk lanes are nodes in the graph -- sidewalks can't ever be
      changed in A/B Street, so there's no concern about reusing node orderings.
    - All turns between two sidewalks become edges, again using length
    - When actually pathfinding, we get back a list of sidewalks. The actual
      paths used in the traffic simulation specify forwards or backwards on a
      sidewalk. Looking at adjacent pairs of sidewalks lets us easily stitch
      together exact directions.
- `make/bus_stops.rs`: Match bus stops with a sidewalk
  - Also precompute the position where the bus stops on the adjacent driving or
    bus lane.
  - This "equivalent position on another lane" process has a few weird cases,
    since two lanes on the same road might have different lengths. Right now,
    the same distance from the start of the lane is used, with clamping for
    shorter lanes. Ideally, the position would be found by projecting a
    perpendicular line out from one lane to the other.
- `make/bus_stops.rs`: Finalize the list of bus routes
  - Between each pair of adjacent bus stops, run pathfinding to verify there's
    actually a path for the bus to follow. If any are disconnected, remove the
    bus route
  - Remove bus stops that have no routes serving them.
- `pathfind/walking.rs`: Precompute the CH for pedestrians who will use buses
  - Nodes in the graph are sidewalks and every bus stop
  - There's an edge with weight 0 between a bus stop and its sidewalk
  - There's also an edge with weight 0 between bus stops that're adjacent via
    some route. Ideally this weight would account for the time until the next
    bus and the time spent on the bus, etc.
  - Later when figuring out which bus to use for a pedestrian, the resulting
    list of nodes is scanned for the first and last bus stop along the same
    route.

## Live edits

A key feature of A/B Street is the player editing the map and seeing how traffic
responds. The possible edits include:

- Change lane types (driving, bus, bike, parking -- sidewalks are fixed)
- Change speed limits
- Reverse a lane
- Change a stop sign policy (which roads have a stop sign and which have
  priority)
- Change a traffic signal policy

The map conversion process outlined above takes a few minutes, so reusing this
process directly to compute a map with edits wouldn't work at all for real
gameplay. Instead, the process for applying edits is incremental:

- Figure out the actual diff between edits and the current map
  - This is necessary for correctness, but also speeds up a sequence of edits
    made in the UI -- only one or two lanes or intersections actually changes
    each time. Of course when loading some saved edits, lots of things might
    change.
- For any changed roads, make sure any bus stop on it have a good pointer to
  their equivalent driving position for the bus.
- For any modified intersections, recompute turns and the default intersection
  policies
- Recompute all the CHs for cars, buses, and bikes -- note sidewalks and bus
  stops never change
  - This is the slowest step. Critically, the `fast_paths` crate lets a previous
    node ordering be reused. If just a few edge weights change, then recomputing
    is much faster than starting from scratch.
  - While making edits in the UI, we don't actually need to recompute the CH
    after every little tweak. When the player exits edit mode, only then do we
    recompute everything.

A list of lanes and intersections actually modified is then returned to the
drawing layer, which uploads new geometry to the GPU accordingly.

## Development tricks

- Separate phases for fast incremental development
  - Don't reimport all data from OSM every time there's a change to part of the
    map construction code!
  - For slow steps that don't change often, make them separate binaries -- hence
    `convert_osm` being separate from the rest.
- Don't be afraid of manual intervention
  - The data isn't perfect. It's easy to spend lots of time fiddling with code
    to automatically handle all problems
  - Instead of automatically resolving problems, prefer good tooling for finding
    and specifying fixes
  - Be careful of derivative structures that could get out of sync with OSM.
    Prefer contributing real fixes to OSM.
- Screenshot diff testing
  - When working on the code for intersection geometry, it's easy to check a few
    example cases get fixed by some change. But what if another part of the map
    regresses somehow?
  - Take screenshots of the entire map, keep the checksums under version
    control, look at the diffs visually, and manually verify any changes.
  - Implementation details: One huge gif or png is too slow to read and write,
    so take a bunch of tiled screenshots covering everything. Amusingly,
    rendering to a file with `glium` is slow unless compiling in release mode
    (which isn't an option for quick incremental development). So instead, pan
    to each section of the map, render it, call an external screenshot utility,
    and move on -- just don't wiggle the mouse during this process!
- Different IDs for objects make sense during different phases
  - For the final product, lanes and such are just a contiguous array, indexed
    by numeric IDs.
  - But sometimes, we need IDs that're the same between different boundary
    polygons of maps, so that player edits can be applied anywhere. Using
    (longitude, latitude) pairs hits floating-point serialization and comparison
    issues, so referring to roads as (OSM way ID, OSM node ID 1, OSM node ID 2)
    works instead.

## Appendix: PolyLines

Add some pictures here to demonstrate how polyline shifting works, the
explode-to-infinity problem, and the bevel/miter fix.
