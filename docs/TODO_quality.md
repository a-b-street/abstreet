# TODO - Map quality

## Geometry

- can natively order geom stuff now!

- underlying problems
	- bad polyline shifting remains
		- from the remaining cases, looks like we need to totally remove some tight points and retry
		- make polygons use the corrections too?

- generalized_trim_back
	- breaks down when we have jagged lane endings due to polyline shift angle correction
	- sometimes a lane polyline hits the perpendicular of a trimmed road! where was this happening?

- handle small roads again somehow?
	- VERY overeager... ate half of the map
		- can we capture snapshots of incremental changes?
			- save initial map at every step, be able to load raw + initial with a focus point
			- generic viewer should be easy... something that stores polygon and ID, wraps the quadtree, etc
		- try merging the shortest roads first
	- deal with loop roads

	- manually draw a picture of the weird intersection to see what would look reasonable. i think we need original road bands from deleted stuff to make decent polygons.

	- model U-turns

- degenerate-2's should only have one crosswalk
	- then make them thinner

- ped paths through sidewalk corners are totally broken
	- calculate the better paths first, then make the corner geometry from that?
- car turns often clip sidewalk corners now

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
- draw ALL water and greenery areas
- draw benches, bike racks
- render trees
- look for current stop sign priorities
	- https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/

## Low-priority geometry issues

- if building front path intersects another building, then scrap that building.
	- or wait, just require bldgs to be even closer to sidewalk first.
	- need to do polygon vs polygon check!
	- will need to speed it up with quadtree containing entire buildings. make sure these are easy to use.

- can we make OSM buildings with holes?
	- experiment with https://docs.rs/clipping/0.1.1/clipping/gh/struct.CPolygon.html and https://github.com/21re/rust-geo-booleanop

## More problems to fix

- Disconnected map
	- Some lane-changing model needed to determine this
- Impossible turns (from a far bus lane to a crazy left)

## Release

- publish the map data
- stabilize the code except for sim
