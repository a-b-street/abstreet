# TODO for Phase 1 (Basemap)

- huge maps
	- manually mark polygon for the part of seattle to simulate
	- maybe need a quadtree for sidewalk finding to actually work (slightly weird, because no render layer -- use center points only)
	- look into all the warnings (trim failing, no driving lane for buses, duplicate turns)

- lots more data
	- lanes: https://data-seattlecitygis.opendata.arcgis.com/datasets/49d417979fec452981a068ca078e7070_3
	- traffic circles: https://data-seattlecitygis.opendata.arcgis.com/datasets/717b10434d4945658355eba78b66971a_6
	- https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
	- https://data-seattlecitygis.opendata.arcgis.com/datasets/curb-ramps
	- high quality thick roads: https://seattlecitygis.maps.arcgis.com/apps/webappviewer/index.html?id=86cb6824307c4d63b8e180ebcff58ce2

- trim parcels that're nowhere near roads (aka, the bbox is kinda wrong)

- maybe also the time to split into different lane types? what's similar/not between them?
	- graph querying?
	- rendering (and other UI/editor interactions)?
	- sim state?

- more data
	- draw ALL water and greenery areas
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
	- figure out what to do about yellow center lines
		- yellow and white lines intersect cars and turn icons and such
		- who should own drawing them?
		- trim them back too (maybe to avoid hitting the intersection?)

- code cleanup
	- move map_model geometry stuff elsewhere (sim stuff also needs it though)
	- merge control map into one of the other layers?

- better drawing
	- detailed turns, like https://i.ytimg.com/vi/NH6R3RH_ZDY/maxresdefault.jpg
	- rooftops
		- https://thumbs.dreamstime.com/b/top-view-city-street-asphalt-transport-people-walking-down-sidewalk-intersecting-road-pedestrian-81034411.jpg
		- https://thumbs.dreamstime.com/z/top-view-city-seamless-pattern-streets-roads-houses-cars-68652655.jpg
	- https://gifer.com/en/2svr
