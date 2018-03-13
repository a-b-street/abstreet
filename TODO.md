# TODO

## mvp strawman sim

- stop signs
- traffic signals
- mouseover cars
- draw next intended turn (and accepted or not state) as blinky arrow
- draw moving / blocked colors (gradually more red as they wait)
- draw stop buffer in front/behind of cars
- cars with different speeds
- clean up code
	- make a headless sim mode, forcing some crate splits
	- split render/ into geom/ too...
	- line type
	- ditch vec2d / settle on types
	- split out UI stuff
	- typesafe seconds, distances, angles, etc
	- easier way to define magic tuneable constants
	- reuse CarIDs...
	- proper timestep handling (https://gafferongames.com/post/fix_your_timestep/)

## editor mode

- make editor mode for traffic signals
	- button to reset intersection to original cycles
	- turns can belong to multiple cycles; the colors become slightly meaningless

- stop sign editor (start simple)
	- cant have no stop signs for two roads whose center line crosses
		- infer default policy
	- draw as red octogon with thin white trim, off to the right side of the road
	- later feature: ban individual turns

- more data
	- shp parser for traffic signals
	- do need to mouseover shapefile things and see full data
	- traffic signal description field says number of phases
	- so many signs: https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/
		- all up north? :(

- why are some icons in the intersection?
- support left turn yield
- draw detailed turns better, like https://i.ytimg.com/vi/NH6R3RH_ZDY/maxresdefault.jpg
- mark problem areas where road is too short!
- manually fix OSM issues, like deleting a way completely

## cleanup

- add/plan tests
- run clippy everywhere
- extract common crates
- ask about mut vs returning new version of self (and what that requires of all the contained stuff)
- break editor up into more crates
- minimize heap usage -- look into profiling and smallvec

## Conga line idea

- try constructive approach for snake idea
	- with interactive mode?
- try destructive approach for snake idea
	- with interactive mode?
- allow for manual tuning
- serialize results

## UI

- support massive maps
	- render to a bitmap and clip that in?
	- drop events sometimes

- draw water and greenery areas

- more toggleable layers
	- show road/bldg types by color
		- can also use to interactively find osm filters to fix
	- show where on-street parking probably is
	- show where sidewalks probably are
		- infer from roads and parcel data?
	- draw benches, bike racks
		- more generally, a way to display random GIS data from seattle site (kml)

- 3D UI sharing the same structure as the 2D one
- web version
