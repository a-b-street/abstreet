# TODO - Map quality

## Boundary clipping

- detect border nodes better by clipping roads based on the desired polygon in convert_osm!
	- still weird!
	- clip areas
	- outgoing border nodes do kind of need a policy... can throttle intake to model backup.

- some areas in small_seattle are borked or missing
	- at least for lakeslice, we have points in good orders, but need to clip areas too.

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
- draw benches, bike racks
- render trees
- look for current stop sign priorities
	- https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/
- population
	- http://seattlecitygis.maps.arcgis.com/apps/MapSeries/index.html?appid=3eb44a4fdf9a4fff9e1c105cd5e7fe27
	- https://data.seattle.gov/Permitting/Rental-Property-Registration-Map/5a7u-vxx7
	- https://www.seattle.gov/transportation/document-library/reports-and-studies
	- https://commuteseattle.com/wp-content/uploads/2017/02/2016-Mode-Split-Report-FINAL.pdf
	- https://www.soundtransit.org/get-to-know-us/documents-reports/service-planning-ridership
	- https://gis-kingcounty.opendata.arcgis.com/datasets/parcels-for-king-county-with-address-with-property-information--parcel-address-area
		- PREUSE_DESC reveals landuse
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

## Release

- publish the map data
