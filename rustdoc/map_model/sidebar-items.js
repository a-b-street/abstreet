window.SIDEBAR_ITEMS = {"constant":[["MAX_BIKE_SPEED",""],["MAX_WALKING_SPEED",""],["NORMAL_LANE_THICKNESS",""],["PARKING_LOT_SPOT_LENGTH","From some manually audited cases in Seattle, the length of parallel street parking spots is a bit different than the length in parking lots, so set a different value here."],["SIDEWALK_THICKNESS",""]],"enum":[["AmenityType","Businesses are categorized into one of these types."],["AreaType",""],["BufferType",""],["BuildingType",""],["CommonEndpoint",""],["Direction",""],["DrivingSide",""],["EditCmd",""],["EditIntersection",""],["IntersectionType",""],["LaneType",""],["OffstreetParking","Represent no parking as Private(0, false)."],["PathConstraints","Who’s asking for a path?"],["PathStep",""],["PathStepV2","One step along a path."],["PathfinderCaching","When pathfinding with different `RoutingParams` is done, a temporary pathfinder must be created. This specifies what type of pathfinder and whether to cache it."],["RestrictionType",""],["SideOfRoad","See https://wiki.openstreetmap.org/wiki/Forward_%26_backward,left%26_right."],["StageType",""],["Traversable","Either a lane or a turn, where most movement happens."],["TurnPriority",""],["TurnType",""]],"mod":[["city",""],["connectivity",""],["edits","Once a Map exists, the player can edit it in the UI (producing `MapEdits` in-memory), then save the changes to a file (as `PermanentMapEdits`). See https://a-b-street.github.io/docs/tech/map/edits.html."],["make","See https://a-b-street.github.io/docs/tech/map/importing/index.html for an overview. This module covers the RawMap->Map stage."],["map","A bunch of (mostly read-only) queries on a Map."],["objects",""],["osm","Useful utilities for working with OpenStreetMap."],["pathfind","Everything related to pathfinding through a map for different types of agents."],["traversable",""]],"struct":[["AccessRestrictions",""],["Amenity","A business located inside a building."],["Area","Areas are just used for drawing."],["AreaID",""],["Block","A block is defined by a perimeter that traces along the sides of roads. Inside the perimeter, the block may contain buildings and interior roads. In the simple case, a block represents a single “city block”, with no interior roads. It may also cover a “neighborhood”, where the perimeter contains some “major” and the interior consists only of “minor” roads."],["Building","A building has connections to the road and sidewalk, may contain commercial amenities, and have off-street parking."],["BuildingID",""],["City","A single city (like Seattle) can be broken down into multiple boundary polygons (udistrict, ballard, downtown, etc). The load map screen uses this struct to display the entire city."],["CompressedMovementID","This is cheaper to store than a MovementID. It simply indexes into the list of movements."],["ControlStopSign",""],["ControlTrafficSignal","A traffic signal consists of a sequence of Stages that repeat in a cycle. Most Stages last for a fixed duration. During a single Stage, some movements are protected (can proceed with the highest priority), while others are permitted (have to yield before proceeding)."],["DirectedRoadID",""],["EditEffects",""],["EditRoad",""],["Intersection","An intersection connects roads. Most have >2 roads and are controlled by stop signs or traffic signals. Roads that lead to the boundary of the map end at border intersections, with only that one road attached."],["IntersectionCluster","This only applies to VehiclePathfinder; walking through these intersections is nothing special. And in fact, even lanes only for buses/bikes are ignored."],["IntersectionID",""],["Lane","A road segment is broken down into individual lanes, which have a LaneType."],["LaneID","A lane is identified by its parent road and its position, ordered from the left."],["LaneSpec",""],["Map",""],["MapConfig",""],["MapEdits","Represents changes to a map. Note this isn’t serializable – that’s what `PermanentMapEdits` does."],["Movement","A Movement groups all turns from one road to another, letting traffic signals and pathfinding operate at a higher level of abstraction."],["MovementID","A movement is like a turn, but with less detail – it identifies a movement from one directed road to another. One road usually has 4 crosswalks, each a singleton Movement. We need all of the information here to keep each crosswalk separate."],["NamePerLanguage","None corresponds to the native name"],["OriginalRoad","Refers to a road segment between two nodes, using OSM IDs. Note OSM IDs are not stable over time."],["ParkingLot","Parking lots have some fixed capacity for cars, and are connected to a sidewalk and road."],["ParkingLotID",""],["Path",""],["PathRequest",""],["PathV2","A path between two endpoints for a particular mode. This representation is immutable and doesn’t prescribe specific lanes and turns to follow."],["Pathfinder",""],["PathfinderCache","For callers needing to request paths with a variety of RoutingParams. The caller is in charge of the lifetime, so they can clear it out when appropriate."],["Perimeter","A sequence of roads in order, beginning and ending at the same place. No “crossings” – tracing along this sequence should geometrically yield a simple polygon."],["PermanentMapEdits","MapEdits are converted to this before serializing. Referencing things like LaneID in a Map won’t work if the basemap is rebuilt from new OSM data, so instead we use stabler OSM IDs that’re less likely to change."],["Position","Represents a specific point some distance along a lane."],["RawToMapOptions","Options for converting RawMaps to Maps."],["Road","A Road represents a segment between exactly two Intersections. It contains Lanes as children."],["RoadID",""],["RoadSideID",""],["RoadWithStopSign",""],["RoutingParams","Tuneable parameters for all types of routing."],["Stage",""],["TransitRoute",""],["TransitRouteID",""],["TransitStop",""],["TransitStopID",""],["Turn","A Turn leads from the end of one Lane to the start of another. (Except for pedestrians; sidewalks are bidirectional.)"],["TurnID","Turns are uniquely identified by their (src, dst) lanes and their parent intersection. Intersection is needed to distinguish crosswalks that exist at two ends of a sidewalk."],["UberTurn",""],["Zone","A contiguous set of roads with access restrictions. This is derived from all the map’s roads and kept cached for performance."]]};