# InitialMap to Map

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
