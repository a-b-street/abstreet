# GUI-related design notes

## Moving away from piston

- options
	- gfx (pre-II or not?) + winit
		- https://suhr.github.io/gsgt/
	- glium (unmaintained)
	- ggez: https://github.com/nical/lyon/blob/master/examples/gfx_basic/src/main.rs
		- too much other stuff

https://www.reddit.com/r/rust_gamedev/comments/7f7w60/auditioning_a_replacement_for_bearlibterminal/

https://github.com/ggez/ggez/blob/master/examples/drawing.rs

things to follow:
	- https://suhr.github.io/gsgt/
	- https://wiki.alopex.li/LearningGfx
	- https://github.com/nical/lyon/blob/master/examples/gfx_basic/src/main.rs

	- porting to wasm: https://aimlesslygoingforward.com/blog/2017/12/25/dose-response-ported-to-webassembly/

## Refactoring UI

I want to make bus stops be a selectable layer, and I want to add lake/park
areas soon. What did
https://github.com/dabreegster/abstreet/commit/b55e0ae263fcfe4621765a7d4a8d208ab5b89e76#diff-8257d0ba4a304de185c0125ff99e353b
have to add?

- color scheme entry
- an ID (displayable)
= boilerplate in selection plugin
	= a way to ask for tooltip lines
- boilerplate in the hider plugin
- DrawExtraShape
	- draw, contains_pt, get_bbox, tooltip_lines
- DrawMap
	- list of stuff, with ID lookup
	- a quadtree and way to get onscreen stuff
- UI
	- a toggleablelayer for it
		= and clearing selection state maybe
	- are we mouseover it? (right order)
	- draw it (right order)
	- pick the color for it

try a quadtree with any type of object.


alright, time to move color logic. let's see what it takes for each Renderable to decide its own color -- what state do they need?
- dont forget: search active and lane not a match does yield a color.
- woops, sim Draw{Car,Ped} doesnt implement Renderable. but it'll be nested anyway... we dont want to move stuff in the quadtree constantly. the 'get everything onscreen' routine should do that, interpreting Lanes and Intersections in initial pass of results correctly by querying for more stuff.
- actually, still undecided about color and RenderOptions...
	- the whole motivation -- one draw() interface can only return a single color. and dont want to add code to UI for every single object type.
		- maybe send in Option<Box<Plugin>> of the current thing, and then it can have a more specific method for each object type? or is that too far the other direction again?
	- send in Option<Color>, letting each plugin vote?
	- maybe if we only have one or two active plugins, it'll be easier to understand?
	- would it be weird to invert and send in all the plugins? the point is
	  kinda for there to be one place to handle interactions between
          plugins -- UI. having a strong concept of one active at a time would probably
          _really_ help.
	- maybe plugins do have a single color_obj(enum of IDs) -> Option<Color>?
- make it easier to fill out RenderOptions for things generically
- next step: extra things like draw_front_path also take cs, not a specific color -- if it makes sense.


- refactor selection plugin's color_something
- selection plugin currently has this weird case where it can cycle through turns at an intersection. MOVE THAT out.
	- more generally, move out all custom logic. make other things know if something is selected and do special stuff if so.
	- and make it act generic at last! \o/


OK, so where was I?
- colors are still currently missing for things that need two of them.
** having one active plugin at a time simplifies the color problem and solves a few others, so try that now.
	- another refactor to do -- initiate plugins based on current_selection_state in UI or the plugin, but stop mixing so much
- make car and ped also Renderable, for great consistency!
- work on generic quadtree idea

### One active plugin at a time

I wonder if the UI will have a need to reach into plugins beyond event(). Let's find out!
- exceptions
	- hider needs to given to finding onscreen stuff
	- search can specify colors and OSD lines
	- warp can add OSD lines
	- show_route can color
	- floodfiller can color
	- steepness can color
	- osm can color
	- signal and stop sign editor can color and indicate what icons are onscreen
	- road editor can be asked for state to serialize
	- sim ctrl can contribute OSD lines (and everything grabs sim from it directly too -- maybe sim should live in UI directly)
	- color picker can draw
	- turn cycler can draw

- the stuff they take in event() is different. hmm.
	- box lil closures

so it feels like we implicitly have a big enum of active plugin, with each of their states kinda hidden inside.

- the simple idea
	- UI keeps having a bunch of separate plugins with their real type
	- have a list of closures that take UI and do event(). return true if that plugin is active
		NOT if something was done with the input
	- in event(), go through the list and stop when something becomes
	  active. remember it's active and just call it directly next time in
          event(), until it says its no longer active.
	- then figure out the implications for color

	tomorrow:
	= implement exclusive active plugin thing
	= rethink if existing plugins are active or not. maybe dont make event() return if active or not, maybe have a separate method? or just in event, do stuff first, and then have this query at the end.
	= tooltips shouldnt be shown while a random plugin is active; move that out to its own plugin! same for debugging. selection plugin should have NO logic.
	= refactor the toggleablelayer stuff, then move them to list of plugins too
	= clean up selection state... should warp and hider be able to modify it, or just rerun mouseover_something?

	= initiate plugins in the plugin's event; stop doing stuff directly in UI
	= basically, make UI.event() just the active plugin list thing as much as possible.
	- deal with overlapping keys that still kinda happen (sim ctrl, escape game)
	- bug: do need to recalculate current_selection whenever anything potentially changes camera, like follow

	= make draw car/ped implement renderable.
		= move Draw* to editor crate, and have an intermediate thing exposed from sim and do the translation in get_blah_onscreen.
	
	- then rethink colors, with simplified single plugin
	= then finally try out a unified quadtree!
		= make parcels selectable as well?

	- and see how much boilerplate a new type would need, by adding bus stops and water/parks


Alright, replan yet again.

= then rethink colors, with simplified single plugin
	= plugin trait, color(id) -> Option<Color>. parallel list of box plugins (or, a fxn that takes the idx)
	= refactor to one color_blah method
	= handle the two color things... just buildings?
= and see how much boilerplate a new type would need, by adding bus stops and water/parks

- load water/parks and stuff
- deal with overlapping keys that still kinda happen (sim ctrl, escape game)
	- and missing keys, like no tooltip for turns, since they're only shown in editors now
- bug: do need to recalculate current_selection whenever anything potentially changes camera, like follow
- consider merging control map into map
- see how hard it is to render textures onto cars or something
= refactor debug and tooltip lines for objects

## Immediate mode GUI

Things organically wound up implementing this pattern. ui.rs is meant to just
be the glue between all the plugins and things, but color logic particularly is
leaking badly into there right now.

## UI plugins

- Things like steepness visualizer used to just be UI-less logic, making it
  easy to understand and test. Maybe the sim_ctrl pattern is nicer? A light
adapter to control the thing from the UI? ezgui's textbox and menu are similar
-- no rendering, some input handling.

## GUI refactoring thoughts

- GfxCtx members should be private. make methods for drawing rectangles and such
	- should be useful short term. dunno how this will look later with gfx-rs, but dedupes code in the meantime.
- should GfxCtx own Canvas or vice versa?
	- Canvas has persistent state, GfxCtx is ephemeral every draw cycle
	- dont want to draw outside of render, but may want to readjust camera
	- compromise is maybe storing the last known window size in canvas, so we dont have to keep plumbing it between frames anyway.


One UI plugin at a time:
- What can plugins do?
	- (rarely) contribute OSD lines (in some order)
	- (rarely) do custom drawing (in some order)
	- event handling
		- mutate themselves or consume+return?
		- indicate if the plugin was active and did stuff?
- just quit after handling each plugin? and do panning / some selection stuff earlier
- alright, atfer the current cleanup with short-circuiting... express as a more abstract monadish thing? or since there are side effects sometimes and inconsistent arguments and such, maybe not?
	- consistently mutate a plugin or return a copy
	- the Optionals might be annoying.

## OpenGL camera transforms

- https://crates.io/crates/cgmath
- https://www.nalgebra.org/cg_recipes/
- https://docs.rs/euclid/0.19.0/euclid/struct.TypedTransform2D.html
- https://www.reddit.com/r/rust/comments/6sukcw/is_it_possible_to_to_create_an_ortho_view_in_glium/ has a direct example of affine transformation
- https://www.reddit.com/r/gamedev/comments/4mn9ly/3d_matrix_transformation_question_rotating/

- https://docs.rs/cgmath/0.14.1/cgmath/trait.Transform.html#tymethod.look_at is ready-made

## Wizard

API looks like coroutines/continuations, but it just works by replaying
previous answers. The caller could even build a branching workflow -- as long
as the calls to the wizard are deterministic.

Menus are super awkward -- drawing extra effects, mainly.

cursive crate is good inspiration for the API

## Menus

Dynamically populating the TreeMenu every frame while possible input keys are collected has problems.

- How do we remember the permanent state between frames?
- What if the possible actions change between frames, screwing up that state anyway?
	- stop handing events to the game entirely?

Rethink assumptions. Is it nice to dynamically populate the menu in a bunch of different places?

- Won't have any control whatsoever for order of entries, and I'll definitely want that.
- Hard to understand all the things that could happen; it'd be nice to see them in one place
- Lots of plugins have boilerplate code for state management. Even if they keep
  it, might be cool to at least peel out the code to activate the plugin.
	- all plugins become Optional; dont have to represent the nil state
	- or more extreme: enum of active plugin
- Similarly, might be nice to have kind of a static list of context-sensitive, right click menu actions for each type of object?




current TreeMenu:
- Debug ()
    - Show extra ()
        - hide Intersection(IntersectionID(59)) (H)
        - start searching (/)
        - to show OSM classifications (6)
        - unhide everything (K)
        - visualize steepness (5)
    - Show layers ()
        - toggle buildings (1)
        - toggle debug mode (G)
        - toggle extra KML shapes (7)
        - toggle intersections (2)
        - toggle lanes (3)
        - toggle parcels (4)
        - toggle turn icons (9)
    - Validate map geometry (I)
    - start searching for something to warp to (J)
- Edit map ()
    - start drawing a polygon (N)
- Settings ()
    - configure colors (8)
- Sim ()
    - Seed the map with agents (S)
    - Setup ()
        - spawn some agents for a scenario (W)
    - load sim state (P)
    - run one step (M)
    - run sim (Space)
    - save sim state (O)
    - slow down sim ([)
    - speed up sim (])
- quit (Escape)



Back up and think about ideal for these background controls...

## Managing different map edits

Should be simple -- I want to bundle together map edits as named things, to
prep for A/B tests. But loading a different set of edits could be kind of
tough...

- new control map state has to propagate to intersection editors.
	- easy fix: pass them mut ref from control map every tick. then just have to reload control map.
- road edits have to propogate
	- theres a way to do that live right now, but it's kind of brittle and funky. probably safer to load from scratch.
		- but then have to reload things like steepness visualizer plugin... actually, just that, seemingly.
		- er, but also the hider plugin -- it holds onto laneIDs, which may change!

Alright, I think this is the sequence of things to do:

1) make a few plugins less stateful anyway, by taking refs to map/control map stuff instead of caching stuff. thats useful regardless.
	- but wait, then road editor kind of cant work, because mut borrow edits from map while holding immutable lane/road refs. theyre really indep things, so cant store together.
2) make it possible to completely reload UI and everything from scratch, from a plugin. rationale: it'd be nice to switch maps from inside the editor anyway. not necessary, but useful.
3) make road edits propogate correctly, and somehow have a strategy for ensuring nothing is forgotten. impl today is VERY incomplete.

Thinking about this again now that we need two copies of everything to be alive at a time and switch between them...

- very tied together: map, control map, draw map, sim
- current selection is UI state that has to refresh when changing maps
- which plugins have state tied to the map?
	- have a method on UI to switch map+edits? no, dont want to reload all this stuff every time...
		- bundle that state together, including the plugins!
		- make the existing 'load new edits' thing use this new mechanism
		- then go back to managing a second sim...

## Rendering a map differently

For "Project Halloween", I want to draw the map model in a very different
visual style. Stuff like intersections are ignored, rendering roads instead of
lanes, and making animated buildings. Let's just start, and see what kind of
common code makes sense to keep.

OK, so far nothing extra to share -- the existing abstractions work well. But
now I think we need a quadtree just to avoid rendering too much stuff. The
Renderable trait from editor is a bit too much -- selection and tooltips, nah.
(Or not yet?) And then since the buildings move, what do we do about the
quadtree? Constantly updating it is silly -- it'd be best to capture the full
extent of the bounding box, the worst case. Actually wait, it's easy -- the
original bbox is fine! The bldg only shrinks closer to the sidewalk when
animating.

So, try adding the quadtree for roads and buildings (diff quadtrees or one
unified? hmm) and see what looks common. Remember we could use quadtrees in map
model construction for building/sidewalk pruning, but there's the awkwardness
of quadtrees _kind of_ being a UI concept.

## Side-by-side

What should this feature do? Is it easier to watch two maps side-by-side moving
in lockstep, with the same camera? Or would a ghostly trace overlay on one map
be easier to understand? The use cases:

- Glancing at a vague overview of how the two runs are doing. Watching graphs
  side-by-side might even be more useful here. But for a zoomed out view,
  side-by-side with reasonably clear pockets of color (weather model style,
  almost) seems nice.
- Detailed inspection of a fixed area. Side-by-side view with full detail seems
  nice.
- Detailed inspection of a specific agent, following it. Side-by-side would
  have to trace it in both canvases.
- Looking for differences... what are these? For a single agent, wanting to
  know are they farther along their journey at some point in time? That could
  be visualized nicely with a red or green thick route in front or behind them.
  Maybe they're ahead of the baseline by some amount, or behind it. This could
  use relative score or relative distance to goal or something. Would it trace
  ahead by pure distance or by anticipated distance in a given time?

The side-by-side canvas seems incredibly difficult -- have to revamp everything
to dispatch mouse events, maybe synchronize cameras, other plugins
arbitrarily... No way.

Let's start with two concrete things:

- Start running an A/B test sets up a second optional simulation in the UI.
  Some keys can toggle between showing one of the two, for now. Stepping will
  step BOTH of them. Have to adjust the OSD and other sim control things.
  Possibly even sim control should own the second sim?
	- Q: how to prevent scenario instatiation or adding new agents while an
	  A/B test is in progress? need plugin groups / modes!
	- road editing during an A/B test is _definitely_ not k
	- argh!! wait, we need a different map, since we need different edits!
		- that means also a different control map
			- should the Sim own map and control_map to make it clear they're tied? I think so!
		- practically all of the UI touches the map...
			- wait wait draw map also needs to be duplicated then.
			- we're back to the problem of loading map edits
- Make a visual trace abstraction to show something in front of or behind an
  agent. It follows bends in the road, crosses intersections, etc. Could be
  used for lookahead debugging right now, and this relative ghost comparison
  thing next.

## Tedious to register a new plugin

- make the plugin (unavoidable, this is fine)
- plugins/mod
- import in ui
- declare it in ui's UI or PerMapUI
- initialize it (can we use default() or something?)
- event handler thing (unavoidable)
	- but maybe Colorizer -> Plugin and make a generic event interface
	- could also put draw there!
- number in the plugin list
- sometimes call draw()

alright, the boilerplate is all gone! \o/ I'm happy now.

## Colors

It's too tedious to make new colors, so I find myself hacking in temporary
hardcoded values. Declaring something far from its use sucks, because there's
no context. So how about this for an alternative:

- allow either [0.0, 1.0] or [0, 255] formats. helper rgb(r, g, b) implicit 1.0 alpha and rgba(r, g, b, a)
- cs.get("name", default)
- the colorscheme object lazily accumulates the list of colors, so the color picker plugin still works
	- but some are in plugins that dont get called often
- what gets serialized? just marked changes from the color picker? yeah, and
  those overrides are loaded in at first. colorscheme obj has a set() fxn that
  remembers it's changed from default in code.
- can still later support different colorschemes with different json files.
- shared colors?
	- should those be defined in one of the places arbitrarily, that we know will be invoked early?
	- could a macro help with registration?
- get() being mutable means we have to use RefCell or propogate mutability in lots of sad places
