# TODO

## Map editor

- still key overlap bugs with floodfilling
	- master UI exclusive state -- one plugin at a time

- traffic signal editor
	- button to reset intersection to original cycles
	- turns can belong to multiple cycles; the colors become slightly meaningless
	- support left turn yield

- stop sign editor
	- cant have no stop signs for two roads whose center line crosses
		- infer default policy
	- draw as red octogon with thin white trim, off to the right side of the road

- better visualization
	- why are some icons in the intersection?
	- draw detailed turns better, like https://i.ytimg.com/vi/NH6R3RH_ZDY/maxresdefault.jpg

## Driving model

- try to simplify straw_model step (less phases?)

- make cars pathfind to their destination

- better visualization
	- draw moving / blocked colors (gradually more red as they wait longer)
	- draw stop buffer in front/behind of cars
	- draw cars in intersections, even when slightly zoomed out
	- draw cars in slightly different colors, to distinguish them better

- start implementing a second AORTAish driving model

- reversible sim

## Map model

- more data
	- parse shp, get traffic signals in the right places
	- do need to mouseover shapefile things and see full data
	- grab number of phases from traffic signal shp
	- look for current stop sign priorities
		- https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/

- multiple lanes
	- prep: get rid of other_side
	- display + mouseover parking lane and sidewalk
		- all roads have sidewalk on both sides, oneways only have parking lane on one side
	- model bikes in driving lanes (as slow cars)
	- add random bike lanes, figure out how turns would work
	- be able to convert between parking and bike lanes, recompute the turns

	- when rendering sidewalks, have an option for a grass buffer

## Code cleanup

- clean up code
	- master Map struct
	- line type / ditch vec2d / settle on types
	- better layering: GeomParcel, GeomIntersection, etc, so that render layer doesn't need to know Bounds and use gps_to_screen

- add/plan tests
- document pieces that're stabilizing
- run clippy everywhere
	- presubmit script
	- also enforce consistent style (import order, extern crate only in mod.rs or lib.rs, derive order)
- extract common crates
- ask about mut vs returning new version of self (and what that requires of all the contained stuff)
	- https://stackoverflow.com/questions/28385339/mutable-self-while-reading-from-owner-object

## Example use cases

- montlake/520 turn restrictions with pedestrian scramble
- close interior neighborhoods to most cars (except for src/dst), see how traffic restricted to arterials would work
- create a bike network with minimal hills, dedicated roads, minimal crossings
