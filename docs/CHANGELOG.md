# CHANGELOG

Every time I upload a new [binary
release](https://github.com/dabreegster/abstreet/releases), I'll list major
changes here.

0.1.0

- First binary release

0.1.1

- drawing arrows better
- start with a splash screen, make it easy to change maps in-game

0.1.2

- totally revamp GUI by organizing everything into distinct gameplay modes

0.1.3

- new warp tool that autocompletes street names
- hideable menus, place context menus better, remove top menu bar, add a simple OSD
- loading screens reflect what's printed to the terminal
- depict pedestrians and bikes with more detail
- tool to scroll through an agent's route
- make simulation speed controls actually work

0.1.4

- improve stop sign editor UI (toggle entire roads)
- better mouseover / selection rendering
- better traffic signal rendering (show time left, use outlines for yields)
- make cars actually stop and briefly wait at stop signs
- improve edit mode diff visualization (cross-hatching)
- render actual stop signs, not just red lines
- fix intersection policies confused by conflicting straight turns with lane-changing
- fix mac scrolling
- better turn indicators
- nicer unzoomed view of roads, with different colors for big/small roads

0.1.5

(release file size jumped from ~15MB to ~70MB because of new PSRC trips)

- improve UX of intersection editors
- define a better set of maps included by default
- improve drawing speed by batching more stuff
- better default traffic signal policies for many cases
- import and visualize census data
- fix missing sidewalks on downtown one-ways
- import and visualize PSRC trip data

0.1.6

- slider widget for controlling time and speed
- fixing bad polyline geometry in most cases; visualizing routes should no longer be buggy
- handle PSRC trips that begin or end out-of-bounds
- draw agents in unzoomed mode in a way simpler way
- improve edit mode: detect reverts to original, easier lane type switching
- lots of fixes for buses: handle edits better, read sequence of stops correctly from GTFS
- set up A/B tests faster

0.1.7

- bulk and revert tools in edit mode
- improve turns and default intersection policies when bike/bus lanes involved
- new tool to manually hint for short roads and weird intersections. some problems have now been manually fixed
- scoreboard of trip results for sandbox and A/B test mode
- reduce lag when sim is running at full speeds, but system is too slow
- switch to easbar's contraction hierarchy crate, making all pathfinding INSANELY fast
- remove weird rules about the world freezing when traffic signals are in "overtime"

0.1.8

- edit mode: convert to a ped scramble cycle, simplify stop sign editor by removing individual turns
- ui: put labels next to sliders, organize modal menus into sections, add a minimize/maximize icon
- A/B test mode: savestate, include time controls and agent following/route tools here
- use more OSM data for turn lanes, turn restrictions from lanes, turn restrictions between entire roads
- dont attempt to cross a traffic signal if there's absolutely no hope
- improve bus route UI tools and make routes using transit more sane
- user-defined shortcuts for jumping between views of a map

0.1.9

- sliders to pick times in wizards
- fix hidpi scaling
- traffic signal diagram scrolls properly
- easier to instantiate a scenario, show all trips involving a building for a scenario
- colorschemes to show trip duration or time blocked
- label buses with route number
- represent overlapping pedestrians as a labeled crowd
- massive performance boost via real priority queue
- prevent cars from "blocking the box"
- prevent all? aborted trips (due to parking blackholes mostly)
- smarter roam-around-for-parking router

0.1.10

- sim
  - parking in off-street garages and on-street lanes on the off-side of oneways now mostly works
  - detect and handle parking blackholes; cars should never get stuck looking for parking now
  - let lower-priority turns happen at traffic signals when higher-priority ones blocked
  - get closer to FCFS ordering at stop signs
  - basic opportunistic lane-changing
  - a bus should be seeded for every route now
- demand data
  - show trips to/from buildings and borders
  - make PSRC trips seed and attempt to use parked cars
- UI
  - different heatmap overlays, like parking availability and busiest areas
  - show colorscheme legends when relevant
  - interactively seed parked cars, spawn more types of trips
  - fix major A/B test mode bug (mismatched scenarios and map edits)
  - adjusting sliders, menu placement, dynamic items
  - consolidating different tools into a single info panel for objects
  - bus route explorer shows entire route, current bus location
- map quality
  - degenerate intersections only have one crosswalk now
  - revamped the map editor for fixing geometry problems, used it in many places
  - nicer yellow center lines (dashed when appropriate)
  - handling OSM turn restriction relations properly
  - fix empty traffic signal phases
  - handling bike lanes on certain sides of the road
  - starting to upstream manually-verified parking lanes into OSM
- new gameplay: reverse direction of lanes

0.1.11

- small UI fixes: fixed width traffic signal diagram, skip info phase of menus when empty
- start drawing (but not using) shared left-turn lanes from OSM
- fix OSM polylines with redundant points (fixing an issue in ballard)
- improved traffic signal policies in some cases
- started upstreaming some sidewalk tags in OSM to fix inference issues
- fixed misclassified right turns
- adjusting map colors
- handling lakes/ocean polygons from OSM way better
- reorganized sim analytics, added stuff for bus arrivals
- adding new internal road points to map editor. almost ready to really aggressively use it
- skipping parking lanes with no nearby sidewalks, since they're unusable
- fix z-order of bridges/tunnels in unzoomed view
- allow unzooming indefinitely
- move lots of sandbox mode controls (and other modes) to menus under buttons and dedicated buttons
- basic support for marking a lane closed for construction
- improved geometry of sidewalks at dead-ends

0.1.12

- reorganize everything as different challenge modes. start implementing 3: optimizing a bus route, speeding up all trips, or causing as much gridlock as possible
- improved bus route explorer
- some UI fixes (popup messages in a few places, moving mouse tooltips to the OSD)
- lots of analytics and time-series plots

0.1.13

- analytics: prebake baseline results properly. hover over plot series. some new modes to see bus network, throughput of a road/intersection over time
- log scale for the speed slider
- add a bulk spawner in freeform mode (F2 on a lane)
- rendering: nicer routes, crosswalks, zoomed car colors
- map data: better stop sign and sidewalk heuristics
- fixed the mac hidpi text rendering issue once and for all?!

0.1.14

- better crosswalk generation when there's only a sidewalk on one side of a road
- edit mode UI revamp: paintbrush-style buttons to apply changes to lanes
- show error messages and prevent edits, like disconnecting sidewalks
- properly ban bikes from highways (revamped rules for vehicles using a lane)
- new freeform mode tool to spawn bikes
- WIP (not working yet): make bikes prefer bike lanes. some debug heatmaps for path cost
- edit mode has proper undo support
