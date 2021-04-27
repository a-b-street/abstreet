initSidebarItems({"enum":[["PathConstraints","Who's asking for a path?"]],"fn":[["zone_cost","Heavily penalize crossing into an access-restricted zone that doesn't allow this mode."]],"mod":[["ch","Uses https://github.com/easbar/fast_paths. Slower creation during map importing, but very fast queries."],["dijkstra","Pathfinding without needing to build a separate contraction hierarchy."],["node_map","Some helpers for working with fast_paths."],["pathfinder",""],["uber_turns","To deal with complicated intersections and short roads in OSM, cluster intersections close together and then calculate UberTurns that string together several turns."],["v1",""],["v2","Structures related to the new road-based pathfinding (https://github.com/a-b-street/abstreet/issues/555) live here. When the transition is done, things here will probably move into pathfind/mod.rs."],["vehicles","Pathfinding for cars, bikes, buses, and trains using contraction hierarchies"],["walking","Pathfinding for pedestrians using contraction hierarchies, as well as figuring out if somebody should use public transit."]],"struct":[["RoutingParams","Tuneable parameters for all types of routing."]]});