initSidebarItems({"enum":[["TurnPriority",""],["TurnType",""]],"fn":[["movement_geom",""]],"struct":[["CompressedMovementID","This is cheaper to store than a MovementID. It simply indexes into the list of movements."],["Movement","A Movement groups all turns from one road to another, letting traffic signals operate at a higher level of abstraction. This is used for pathfinding and traffic signals currently; other places focus instead on turns."],["MovementID","A movement is like a turn, but with less detail -- it identifies a movement from one directed road to another. One road usually has 4 crosswalks, each a singleton Movement. We need all of the information here to keep each crosswalk separate."],["Turn","A Turn leads from the end of one Lane to the start of another. (Except for pedestrians; sidewalks are bidirectional.)"],["TurnID","Turns are uniquely identified by their (src, dst) lanes and their parent intersection. Intersection is needed to distinguish crosswalks that exist at two ends of a sidewalk."]]});