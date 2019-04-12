# A/B Street Map Design

link to code

- diagram of data sources and stages
- explanation of intermediate formats
- autogenerate diagrams of the data schemas
- list invariants
- edits

## Features

demonstrate all of these things with before/after pictures or GIFs showing
functionality

- Lanes
  - show OSM way with metadata
  - then multiple lane types, between two intersections. example with a curved
    road.
  - individual parking spots modeled
  - bus/bike lane restrictions modeled, with connecting turns that make sense
    (bus can go from restricted to general lane, but normal car cant enter a bus
    lane)
- Intersections
  - OSM model doesn't explicitly have these at all; just ways with shared nodes
  - they have geometry; cars and peds stop and wait at the end of a lane, then
    cross through the intersection
    - crosswalks and sidewalk geometry continues
  - turns connect lanes, the turns have a path. turns conflict or don't.
  - stop signs (some directions stop and others dont), traffic signals with
    multiple phases
    - "reasonable" defaults inferred, editor for the rest
  - WIP: small intersections, roundabouts merged
- Buildings
  - classified by use, notion of residential density
  - front path connecting to a sidewalk
- Clipping / borders (WIP)
  - clipping polygon actually chops all geometry to fit -- doesnt just omit
    things that cross or leave dangling OOB stuff
    - cleans up areas (parks, bodies of water) nicely
  - roads that cross the boundary have direction-aware 'border intersections' at
    the end, to model traffic flowing in or out of an area

## Model

The map model is designed for A/B Street's traffic simulation and player
editing, but it likely has other uses.

### Objects

- Road
  - Goes between two Intersections
  - Contains children Lanes in two directions
  - Geometry: a PolyLine representing the yellow line separating the directions
    of travel
    - This is usually the center of the road, except for one-ways or when a road
      has more lanes in one direction.
  - Metadata from OSM
- Lane
  - Belongs to a parent Road
  - Has a LaneType: Driving, Parking, Sidewalk, Biking, Bus
    - Buses and bikes can usually use Driving lanes, but Biking and Bus lanes
      are restricted.
  - Geometry: a PolyLine representing the center of the lane
  - Sidewalks know which Building paths are connected and

borders

<!--
![Alt text](https://g.gravizo.com/svg?
  digraph G {
    Road -> Intersection [label="connects two"];
    Road -> Lane [label="contains"];
    Lane -> Building [label="link to"];
    Lane -> BusStop [label="contains"];
    Intersection -> Turn [label="contains"];
    Turn -> Lane [label="connects"];
    BusRoute -> BusStop [label="connects"];
    Area;
  }
)
-->

![Alt text](https://g.gravizo.com/svg? digraph G { Road -> Intersection
[label="connects two"]; Road -> Lane [label="contains"]; Lane -> Building
[label="link to"]; Lane -> BusStop [label="contains"]; Intersection -> Turn
[label="contains"]; Turn -> Lane [label="connects"]; BusRoute -> BusStop
[label="connects"]; Area; } )

### Edits

### Coordinate system

### Invariants

- min length for lanes, turns
- length for related lanes (sidewalk spot / parking / driving) matching up
- connectivity
  - from any sidewalk to any other
  - from any driving lane to any other
  - no lanechanging needed, because of the turns that exist
- no loop lanes (same src and dst endpt)... but what about cul-de-sacs then?
- associated lanes
  - parking lane or bus stop without driving lane
- all turns renderable by draw_full (making thick polygons can fail)

### Limitations

things not represented

- shared left turn lanes

### Data format

## Data sources

The
[import script](https://github.com/dabreegster/abstreet/blob/master/import.sh)
is the source of truth.

- http://download.bbbike.org/osm/bbbike/Seattle/Seattle.osm.gz
  - OpenStreetMap extract
- https://dds.cr.usgs.gov/srtm/version2_1/SRTM1/Region_01/N47W122.hgt.zip
  - Elevation data, currently unused in the simulation
- https://metro.kingcounty.gov/GTFS/google_transit_2018_18_08.zip
  - Bus stops and routes
- https://data.seattle.gov/Transportation/Traffic-Signals/nr6x-wnd5
  - Location of traffic signals
- https://github.com/seattleio/seattle-boundaries-data/raw/master/data/neighborhoods.geojson
  - Neighborhood boundaries
- http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface
  - Blockfaces, used to determine where on-street parking lanes are
- https://data-seattlecitygis.opendata.arcgis.com/datasets/residential-building-permits-issued-and-final
  - Number of units per residential building

## Conversion process

- raw_data::Map
- HalfMap
- Map

### convert_osm: from data sources to raw_data::Map

- load elevation into memory from a .hgt
- get raw OSM ways from a .osm
- split up OSM ways into roads and intersections
- remove completely disconnected roads
- calculate bounds
- use blockface KML to match parking categories to nearest road and side of the
  road
- match traffic signals from KML to nearest intersection
- load raw bus routes from GTFS
- extract in-bounds neighborhoods from the GeoJSON

### make::half_map: from raw_data::Map to HalfMap

- transform raw intersections to final Intersection struct, and build a map from
  intersection Pt2D to intersection
- transform raw roads to real roads and, using OSM metadata and player edits,
  lanes in both directions, with shifted center lines
- mark border intersections based on metadata and number of incoming and
  outgoing lanes
- create the initial intersection polygons by treating each connected road as a
  thick PolyLine and finding overlaps between adjacent PolyLines
- trim lane center points back by intersecting with intersection polygons
- create all of the turns between incoming and outgoing lanes at each
  intersection
- (unstable and disabled as of 12/21/18) destroy roads shorter than 15m by
  merging intersections and creating longer turns
- recreate each intersection polygon by using the lanes' thick polyline
  endpoints

### Map::create_from_raw: from HalfMap to Map

- assign all turns a numeric index for easy text entry in the UI
- match all bus stops with a sidewalk and adjacent driving/bus lane, and filter
  bus routes based on stops that were successfully assigned
- initialize stop sign-governed intersections with reasonable defaults for which
  roads stop and yield
- initialize traffic signal-governed intersections with reasonable defaults for
  phases
- override intersection defaults with player edits
- create all buildings, matching them to the nearest sidewalk with a front path
- copy over areas (like parks and bodies of water)
- filter out any bus routes with adjacent stops that fail pathfinding

## Conversion tricks

- early roundabout handling
- screenshot diff testing
- stable IDs
- short road trimming
- highway ramp special cases
- intersection polygon calculations
- polyline shifting
- sidewalk corner and crosswalk rendering
- clipping to a boundary polygon
