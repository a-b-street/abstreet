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

0.1.15

- minor bugfixes with reverting lane types, preserving stop signs
- incorporate edits into the challenge splash screen, make sure edits are reset when appropriate
- starting a new challenge mode, just focused on traffic signals
- can't leave traffic signal editor with missing turns
- render pedestrian crowds on building front paths
- traffic signals support an offset parameter
- traffic signal visualization and editing revamped to group related turns together
- can preview traffic using a signal from the editor
- actually apply priority at intersections, so protected turns get first dibs over yield turns

0.1.16

- fix Mac crashing with texture limit bug by switching to texture arrays
- fix crashing simulation when a border intersection was used
- started to implement a new UI design for starting the game

0.1.17

- more work on the pre-game UI, with some flexbox layouting
- prototype a minimap in sandbox mode. doesn't pan or scroll yet.
- prototype a new speed/time control panel from the mockup
- nicer time warp loading screen
- record and show detailed trip timeline, including time to park

0.1.18

- map data: infer more building addresses
- some analytics on how long people spend parking and intersection delay over time
- create an options panel, allowing runtime customization of color scheme, traffic signal rendering, etc
- internal changes to map building pipeline to make it much easier for new devs to onboard
- organizing challenges into sub-stages, starting to flesh out specifics for the fix traffic signal track
- much more realistic pedestrian pathfinding
- fix minimap on mac (dpi issues)
- visual tweaks to cars to make front/back easier to distinguish
- internal change to switch most assets from PNG to SVG

0.1.19

- some challenge modes show a histogram for counting faster/slower trips
- new visualization of current demand per direction at a traffic signal
- implementing some of Yuwen's UI changes: agent counter, split time/speed panel, moved functionality out of the old drop-down menus into a bottom-left tool panel, hiding debug functionality
- replaced right-click context menus with left click to open info panels
- fixed random issues reported by people from HN

0.1.20

- moved some UI functionality around, pulling graphs into info panel
- interactive legend for the minimap, toggle visibility of different agents
- nicer colors and shapes for cars
- misc simulation bugfixes that might help huge_seattle
- pedestrians choose to use transit more realistically, factoring in time for the bus to drive

0.1.21

- switch some analytics dashboards to use buttons, not old non-scrolling menus
- scrollbars... at least a start
- preview traffic signal changes from live sim as the base
- traffic signal preview has normal time/speed controls
- traffic signal editor has undo support
- minimap has buttons to pan

0.1.22

- minimap zoom controls
- traffic signal rendering overhaul
- heatmap colors improved, heatmap appears on minimap
- bus info panel, a start to live delay analytics

0.1.23

- UI revamps: speed panel, minimap controls, heatmap chooser
- bus timeline
- hide internal IDs normally
- limit map zoom
- fix bugs with crosswalks conflicting with vehicle turns

0.1.24

- overhaul traffic signal editor UI, and add redo support
- update main edit mode UI, and add redo support
- limit max unzoom
- fix the infamous HiDPI bug once and for all; minimaps should work everywhere
- almost bug-free support for floating, horizontally and vertically scrolling panels
- overhaul top-center panel, rename scenarios to be less confusing
- expose bus analytics outside of challenge mode
- live info panel can exist during a running simulation
- consolidated agent route/trip information into info panel

0.1.25

- overhauled the tutorial
- tuned top-center panel for sandbox and challenge modes
- make bike and bus lanes more obvious
- show map edits as an overlay anywhere
- tune info panel contents, and show relationships between parked cars and buildings
- fixes to traffic signal editor, like making all-walk conversion idempotent
- nicer throughput and delay plots (sliding windows, grid lines)

0.1.26

- tutorial improved in a few places
- map data: thinner sidewalks, associate buildings with named amenities
- traffic model: vehicles can spawn on all lanes from a border
- much better gameplay speed (previously was too fast)
- UI tuning: lane editor, minimap, signal editor, heatmap legends don't overwrite minimap
- traffic signal challenge communicates score more clearly

0.1.27

- edit mode revamped: click to edit stuff. no more lane paintbrushes. autosaving and save as.
- tutorial: can quit and resume tutorial now
- challenge picking flow simplified
- UI: layouting fixes to full-screen / into stuff, popup menus go beneath buttons, plots improved
- internal change to render all text using vector graphics. other than a few text layouting issues, shouldn't be noticeable, except now tooltips in plots don't get covered up
- misc perf improvements (cache SVGs, drawing many circles for unzoomed agents, dont reload prebaked data)
- upgraded winit, glutin, glium -- hopefully no new bugs introduced on any platforms

0.1.27a

- patch to fix a crash with empty text dimensions on things like building info panels

0.1.28

- all info panels revamped
- some tutorial stages are much more clear, with an updating goal
- traffic signal scorecard generalized to work for some tutorial too
- adjust how selected agents look
- X button on popup menus

0.1.29

- new tool to convert between stop signs and traffic signals
- lane editor easier to edit multiple lanes
- info panels: IDs, mostly avoid horizontal scrolling, better info about trips to/from somewhere, move buttons up
- traffic signal editor UI overhaul
- different data in top-right agent meters panel
- tooltips to communicate keybindings better
- new jump-to-time panel, showing when rush hours occur
- speed controls use more useful speeds
- include ongoing trips in measured trip times
- jump to next challenge after completing one
- lots of tutorial tweaks

0.1.30

- show additional info about traffic patterns and buggy maps
- revamp tutorial UI to group tasks and messages better
- handle different mode transitions when info panel open on an agent
- select entire roads in unzoomed edit mode
- show total time an agent has spent moving / blocked
- use 2-phase traffic signals by default, making the 23rd map successfully complete!
- jump-to-time now optionally points out traffic jams forming
- challenge splash screen improved

0.1.31

- overhauled trip timeline in agent info panels
- overhauled traffic signal details panel and the per-lane turn explorer
- settings page: show all options at once. add way to scale up text/UI elements for high-DPI displays, and an alternate pan/zoom control scheme
- traffic signal edits can now be exported and used in any slice of Seattle. will be using this to hand-map many of them.
- many small tutorial fixes

0.1.32

- some UI work on giving focus to textboxes, improving dropdown menus
- road/intersection plots display baseline sim data too
- start associating people with multiple trips, exposing this a little in the UI
- bring back elevation data, introduce a new overlay. the elevation data is still really bad.

0.1.33

- new "population" overlay, showing people (not just current trips). heatmap and dot map to visualize.
- improved the "delay" overlay to handle roads and intersections
- removed the confusing and useless alternate color schemes for agents
- initial left-hand driving side, tested in Perth, also drawing more arrows for all one-way roads
- loads of internal GUI code refactorings, preparing for a standalone release of the library
- fixed z-buffering and alpha values for web backend

0.1.34

- info panels have been totally overhauled again. multiple tabs, way more clear representation of agents, trips, and people.
- draw people inside of a building
- applied consistent typography everywhere
- lots of internal refactoring

0.1.35

- more info panel work, particularly for trips and buses. change plot settings live.
- prototype of a SEIR pandemic model based on time spent in shared spaces, by orestis
- slight heatmap improvements, more coming
- more typography changes
- mouse cursor now changes for buttons and dragging!
- overhaul minimap controls, make layers behavior zoomed in a little better
- new speed panel and jump-to-time modal

0.1.36

- overhauled simulation data page, with a table to find slow trips and some initial summary visualizations
- plots can change windowing and show/hide series
- layers: fade map to contrast more, better scales/legends
- show relative trip times in info panels
- tools to rewind/ffw to watch particular trips
- refocusing efforts on challenge modes; level 1 of a new one is pretty much ready
- some simulation fixes around parking and a corner case of cars temporarily forming a cycle
- orestis improved the population/pandemic heatmaps
