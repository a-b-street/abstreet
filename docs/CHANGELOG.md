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
