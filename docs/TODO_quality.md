# TODO - Map quality

## Boundary clipping

- lakes missing from huge_seattle
	- just connecting the ends of ways doesnt always work well
	- maybe increase the Bounds for areas, and let clipping clean up later?
- some border intersections have weird OOBish geometry, or the arrows look weird
- simplify border node detection, only do it in convert_osm?

## Geometry

- bad polyline shifting remains
	- from the remaining cases, looks like we need to totally remove some tight points and retry
	- make polygons use the corrections too?
	- bad polyline shifting causes jagged lane endings in generalized_trim_back

- handle small roads again somehow?
	- reduce degenerate min trim. the disabled fix doesn't look great.
	- I40, I25, I0 cut corners when merged. disabled fix works, but breaks other things.
	- try it bigger
	- deal with loop roads?
	- model U-turns

- car turns often clip sidewalk corners now
- draw SharedSidewalkCorners just around the ped path, not arbitrarily thick
	- dont forget to draw the notches

- figure out what to do about yellow center lines
	- intersections on one-ways look weird
	- yellow and white lines intersect cars and turn icons and such
	- who should own drawing them?

## More data

- lanes: https://data-seattlecitygis.opendata.arcgis.com/datasets/49d417979fec452981a068ca078e7070_3
	- not filled out for most streets
- traffic circles: https://data-seattlecitygis.opendata.arcgis.com/datasets/717b10434d4945658355eba78b66971a_6
- https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
	- disagrees with OSM road centers sometimes
- https://data-seattlecitygis.opendata.arcgis.com/datasets/curb-ramps
- high quality thick roads: https://seattlecitygis.maps.arcgis.com/apps/webappviewer/index.html?id=86cb6824307c4d63b8e180ebcff58ce2
- OSM has footways
	- but theyre not marked everywhere
	- and theyre hard to associate with roads (sometimes need to infer a planter strip)
- draw benches, bike racks
- render trees
- look for current stop sign priorities
	- https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/
- http://guides.lib.uw.edu/research/gis/uw-lib_data has cool stuff, but .lyr??

## Low-priority geometry issues

- if building front path intersects another building, then scrap that building.
	- or wait, just require bldgs to be even closer to sidewalk first.
	- need to do polygon vs polygon check!
	- will need to speed it up with quadtree containing entire buildings. make sure these are easy to use.

- can we make OSM buildings with holes?
	- experiment with https://docs.rs/clipping/0.1.1/clipping/gh/struct.CPolygon.html and https://github.com/21re/rust-geo-booleanop

## More problems to fix

- Disconnected map
	- now that LCing model is simple...
- Impossible turns (from a far bus lane to a crazy left)
- Buildings intersecting roads, probably because bad lane inference
	- when this happens, get rid of parking lanes first (one or both sides?)
	- iterative process... have to redo affected roads and intersections
	- we havent filtered buildings by proximity to sidewalk yet
		- if we dont filter at all, we pick up some houseboats! :) should draw water...
- undo disabled traffic signal assertion

## Map edits

- lane type can affect border intersections
- lane type can affect turn idx
	- assert turns are the same
- crash when loading some edits (priori->bikelanez)
	- edit an intersection first, then change lanes to destroy turns. need to recalculate the policy, preserving edits!
		- just revert intersection and warn
		- or store overrides more granularly and warn or do something reasonable

## Sim bugs/tests needed

- do bikes use bike lanes?
- test that peds will use buses organically
	- make sure that we can jump to a ped on a bus and see the bus
- park/unpark needs to jump two lanes in the case of crossing a bike lane or something
	- should only be able to park from the closest lane, though!
- explicit tests making cars park at 0 and max_dist, peds walk to 0 and max_dist
- lanechange rebalancing

## Discrete-event sim model

- cleanup after the cutover
	- proper intersection policies, by seeing full view
	- time travel mode can be very smart
	- dupe code for initially spawning vs spawning when a trip leg starts.
		- validation (just do it in spawn and return an error?)
		- getting the path
		- converting goal into router and such

- perf
	- dig into individual events, still too many?
		- for laggy heads, often round down and try slightly too early
