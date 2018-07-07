# TODO for Phase 1 (Basemap)

- model bike lanes

- more data
	- draw water and greenery areas
	- draw benches, bike racks
		- more generally, a way to display random GIS data from seattle site (kml)
	- render trees
	- parse shp, get traffic signals in the right places
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

- code cleanup
	- rename Road to Lane
	- make final Map serializable too
		- useful to precompute sidewalk paths
		- waiting on https://github.com/paholg/dimensioned/issues/31 to release
	- move map_model geometry stuff elsewhere (sim stuff also needs it though)
	- also a polygon struct? for parcels and buildings. maybe have a form that's pre-triangulated?
	- isolate vec2d

- figure out what to do about yellow center lines
	- yellow and white lines intersect cars and turn icons and such
	- who should own drawing them?
	- trim them back too (maybe to avoid hitting the intersection?)
	- osm tags and such would ideally be part of a master road

- draw detailed turns better, like https://i.ytimg.com/vi/NH6R3RH_ZDY/maxresdefault.jpg

## Intersection geometry brainstorm

- can we merge adjacent polylines at intersections based on closest angle, and then use the existing stuff to get nice geometry?
	- i think we still have to trim back correctly
	- first figure out all the trimming cases for the T, outside and inside lanes, etc


- aha, big bug! we only try to trim first/last lines. do the whole polyline.
	- can think of an easy fixpoint approach to try first, even though it's inefficient.
	- wait, the fixpoint is also incorrect. :(

- before trimming back lines, project out the correct width. sort all those points by angle from the center. thats the intersection polygon? then somehow trim back lines to hit that nicely.
- do the current trim_lines thing, but with lines, not segments? no, there'd be many almost-parallel lines.

- at a T intersection, some lines aren't trimmed back at all

- https://www.politesi.polimi.it/bitstream/10589/112826/4/2015_10_TOPTAS.pdf pg38

- just make polygons around center lines, then intersect?
