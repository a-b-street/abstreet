# TODO for Phase 1 (Basemap)

- lots more data
	- lanes: https://data-seattlecitygis.opendata.arcgis.com/datasets/49d417979fec452981a068ca078e7070_3
	- traffic circles: https://data-seattlecitygis.opendata.arcgis.com/datasets/717b10434d4945658355eba78b66971a_6
	- https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
	- https://data-seattlecitygis.opendata.arcgis.com/datasets/curb-ramps
	- high quality thick roads: https://seattlecitygis.maps.arcgis.com/apps/webappviewer/index.html?id=86cb6824307c4d63b8e180ebcff58ce2

- trim buidings and parcels that're nowhere near roads (aka, the bbox is kinda wrong)

- maybe also the time to split into different lane types? what's similar/not between them?
	- graph querying?
	- rendering (and other UI/editor interactions)?
	- sim state?

- more data
	- draw water and greenery areas
	- draw benches, bike racks
		- more generally, a way to display random GIS data from seattle site (kml)
	- render trees
	- do need to mouseover shapefile things and see full data
	- grab number of phases from traffic signal shp
	- look for current stop sign priorities
		- https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/

- polish geometry
	- new center polylines
		- explode out at some angles
	- draw intersections at dead-ends
	- shift turn icons and stop markings and such away from crosswalk
	- interpret failed shifting/polyline attempt as implying something about the lane specs
	- some bldg paths are quite long.
	- sidewalk paths start in building centers and end in sidewalk centers
		- this is probably fine to show agents moving, but at least draw
		  building layer before sidewalk layer
	- figure out what to do about yellow center lines
		- yellow and white lines intersect cars and turn icons and such
		- who should own drawing them?
		- trim them back too (maybe to avoid hitting the intersection?)

- code cleanup
	- move map_model geometry stuff elsewhere (sim stuff also needs it though)
	- also a polygon struct? for parcels and buildings. maybe have a form that's pre-triangulated?
	- isolate vec2d

- draw detailed turns better, like https://i.ytimg.com/vi/NH6R3RH_ZDY/maxresdefault.jpg

## Intersection geometry brainstorm

- can we merge adjacent polylines at intersections based on closest angle, and then use the existing stuff to get nice geometry?
	- i think we still have to trim back correctly
	- first figure out all the trimming cases for the T, outside and inside lanes, etc


- before trimming back lines, project out the correct width. sort all those points by angle from the center. thats the intersection polygon? then somehow trim back lines to hit that nicely.
- do the current trim_lines thing, but with lines, not segments? no, there'd be many almost-parallel lines.

- at a T intersection, some lines aren't trimmed back at all

- https://www.politesi.polimi.it/bitstream/10589/112826/4/2015_10_TOPTAS.pdf pg38

- just make polygons around center lines, then intersect?






morning thoughts!

- trim lines based on outermost POLYGON border line, not lane center lines or anything
- the ascending angle and skipping existing lines in the thesis seems to make sense
- find where infinite line intersects line segment for some cases?
