# TODO - Map quality

## Geometry

- first, stop doing make_old_polygon entirely. figure out the cases where make_new_polygon fails.
	- look at i15 and i98 next
		- watch out, road centers look partly trimmed on i98
		- almost just treat them as degenerates

	- automatically find problems
		- for every road band, find the two endpoints. make sure they
		  exactly match one edge of the intersection polygon.

		- more general solver... for merged intersections, just take
		  perp endpoints of stuff if they dont touch anything else.
		  combination of trimming back road centers and taking perpendiculars.
	- maybe oneway shifting is wrong? do those OSM pts represent the center of the one-way?

- degenerate-2's should only have one crosswalk
	- then make them thinner

- handle small roads again somehow?
	- what's correct for 14th and e boston? if we had less lanes there, would it help?

	- make the polygons for the merged intersections look better
	- same for the sidewalk corners
	- make sure the turns are reasonable
	- apply the merge automatically somehow

	- or retry the later-phase intersection merging
		- kind of need the ability to step through and see each stage...
		- composite turns have inner loops!
		- deal with all TODOs (like sidewalks)

	- model U-turns

- ped paths through sidewalk corners are totally broken

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
