# Backlog

These are very old TODOs that aren't captured elsewhere.

## Themed rendering

- halloween, winter, jungle, 8bit, organic (deform buildings), floorplan,
  machine (make buildings pump along front paths)

## Halloween visual demo

- cars with headlights
- pumpkins
- fences / bushes / features in houses or triangulation on a block with buildings
- buildings as rooms in a hotel
- silent hill soundtrack
- deformed buildings pulsing on front path springs
- lighting?
- fog effects
- in 3D, what if roads are sunken rivers and buildings giant skyscrapers?
- eyes on the houses, that blink
- trick-or-treaters wandering around

## Conga line idea

- try constructive approach for snake idea
	- with interactive mode?
- try destructive approach for snake idea
	- with interactive mode?

## Charm

- music / sound effects
	- as you zoom in, overhear conversations and such
- some buildings could have extra detail
- zoom in too much, what might you see? ;)
- loading screen: snakey cars
- game intro/backstory: history of seattle urban planning
- player context: a drone. people will look up eventually.

## More things to simulate

- seed parked cars in neighborhood with no owner or a far-away owner, to model reasonable starting state
- outgoing border nodes can throttle to simulate traffic downstream

## Tooling

- play with https://github.com/glennw/thread_profiler
- and https://github.com/ferrous-systems/cargo-flamegraph
- display percentage breakdowns in Timer (need tree structure)

## Boundary clipping

- some border intersections have weird OOBish geometry, or the arrows look weird
- simplify border node detection, only do it in convert_osm?

## More data

- lanes: https://data-seattlecitygis.opendata.arcgis.com/datasets/49d417979fec452981a068ca078e7070_3
	- not filled out for most streets
- traffic circles: https://data-seattlecitygis.opendata.arcgis.com/datasets/717b10434d4945658355eba78b66971a_6
- https://data-seattlecitygis.opendata.arcgis.com/datasets/sidewalks
	- disagrees with OSM road centers sometimes
- https://data-seattlecitygis.opendata.arcgis.com/datasets/curb-ramps
- high quality thick roads: https://seattlecitygis.maps.arcgis.com/apps/webappviewer/index.html?id=86cb6824307c4d63b8e180ebcff58ce2
- render trees
- look for current stop sign priorities
	- https://gis-kingcounty.opendata.arcgis.com/datasets/traffic-signs--sign-point/
- http://guides.lib.uw.edu/research/gis/uw-lib_data has cool stuff, but .lyr??

## Map edits

- lane type can affect border intersections

## Sim bugs/tests needed

- do bikes use bike lanes?
- test that peds will use buses organically
	- make sure that we can jump to a ped on a bus and see the bus
- park/unpark needs to jump two lanes in the case of crossing a bike lane or something
	- should only be able to park from the closest lane, though!
- explicit tests making cars park at 0 and max_dist, peds walk to 0 and max_dist
- lanechange rebalancing
- parking/unparking on offside of oneway

## Laundry list of intersection geometry ideas

- make sure road widths are reasonable first
	- SDOT dataset
	- channelization
- extend all the thick roads until they poke out of stuff (except for roads continuing straight)
- play with https://github.com/w8r/polygon-offset
- https://github.com/migurski/Skeletron
- stitch together orig center line of adj roads. then do polyline shifting, which already handles angle eating?
- manually draw intersections
	- montlake/520 4 traffic signal case. existing road geometry in OSM doesn't even cover everything.

## Map layer

- fixed precision math
	- more careful geom types, with negative/positive cases
	- also bounds?
	- cant get rid of the ccw intersection check... different answer in some cases that looks bad

## Sim layer

- rename Car->Vehicle?
- spawning is convoluted
	- popdat trip -> Scenario SpawnTrip -> pick ped speed and make spawner's TripSpec -> create trip and schedule a Command -> last minute rewriting when executing the command
- more precise car FSM by putting scheduler pointer into carstate

## ezgui layer

- probably use f32, not f64 everywhere... but after Pt2D becomes fixed size
- undo the y inversion hacks at last!
- ezgui passes EventCtx and DrawCtx with appropriate things exposed.
	- hide stuff inside the ctx's? canvas and prerender shouldnt even be known outside of crate
- loading screen
	- FileWithProgress should go directly into Timer
		- need to understand lifetimes

## Fix existing stuff

- if a lane could feasibly have multiple turn options but doesnt, print "ONLY"
- text box entry: highlight char looks like replace mode; draw it btwn chars

## New features

- collapse smaller roads/neighborhoods and just show aggregate stats about them (in/out flow, moving/blocked within)

## Better rendering

- depict residential bldg occupany size somehow
- rooftops
	- https://thumbs.dreamstime.com/b/top-view-city-street-asphalt-transport-people-walking-down-sidewalk-intersecting-road-pedestrian-81034411.jpg
	- https://thumbs.dreamstime.com/z/top-view-city-seamless-pattern-streets-roads-houses-cars-68652655.jpg
- general inspiration
	- https://gifer.com/en/2svr
	- https://www.fhwa.dot.gov/publications/research/safety/05078/images/fig6.gif
	- http://gamma.cs.unc.edu/HYBRID_TRAFFIC/images/3d-topdown.jpg
- color tuning
	- neutral (white or offwhite) color and make noncritical info close to
	  that. http://davidjohnstone.net/pages/lch-lab-colour-gradient-picker,
          chroma < 50
