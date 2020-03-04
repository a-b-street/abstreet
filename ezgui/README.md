# ezgui notes

I'm considering cleaning this thing up and releasing a new crate.

## Features

Runs in lots of places. Specifically:

- Linux, Mac, Windows via glium
- wasm via glow. Text support coming in O(months).
  https://github.com/RazrFalcon/resvg/issues/229
- Dealing with DPI sucks. There's one place to set a scale factor, then have all
  text and widgets and stuff scaled up.

Fully vector.

- usvg -> lyon -> polygons
- Load an svg button, programatically swap colors, rotate, scale, etc
- Even text!

Lots of orthogonalish stuff that probably doesn't belong in a GUI library, but
is darn useful. I didn't design this as a GUI library, I just made exactly the
stuff I need for A/B Street.

- Plots and histograms
- Screenshot entire map in chunks for visual diff tests

The canvas. Concept of map- and screen-space. Nice 2D panning and zooming.

Composites and ManagedWidgets.

- flexbox styling via stretch, although I don't understand when margin/padding
  doesnt work.
- buttons, checkboxes (wrapper over buttons), sliders, pop-up menus, fillers for
  custom magic, plots, histograms, extremely basic text entry

Weird API. No callbacks. You manually plumb events to widgets, ask about
results. No smart diffing of widget trees; usually recreate everything from
scratch (it's perfectly cheap in my use cases) or replace a widget in a
composite.

Wizard... though I'm not sure about this pattern. Been switching to settings
panels that ask you lots of stuff at once, give defaults.

## Design choices

NOT performance critical. When I spot something causing noticeable problems, I
profile and fix it reactively. There's so much waste everywhere, like constantly
recreating widgets from scratch and re-uploading similar geometry over and over.
The perf is totally fine for my (reasonably complicated) use cases; if you find
something that doesn't work for you, let's look at it.

Messy error handling. unwraps and panics in different places.

Backend impl is minimal. No traits, plumbing generics -- why would you ever
compile in more than one option anyway?

One (simple) uber-shader. Don't do anything fancy in the shader, calculate
geometry on the CPU. We can use the same geometry for drawing and detecting
mouseover.

## Major problems

Type-safe Outcomes via low-boilerplate enums?

Configuring default style

## FAQ

### Why OpenGL?

My requirements are super simple; I don't need the power of Vulkan or the new
stuff. I want something simple that runs everywhere. If you want to make this
work with WGPU or something else, it should be easy. The 3 backend
implementations (glium, glow on native, glow on wasm) are each about 300 lines.

### Why another? Shouldn't we consolidate?

We should. I didn't particularly want to write a GUI library; nothing did what I
wanted when I started. I don't know if it's worth polishing this up and making
it a real contender in the Rust GUI space, or efforts should be consolidated in
another library.

### Comparison to alternatives

Huge list here. Notably, why not iced? Biggest blocker is that I can't run the
examples today; there's some bug with the wgpu support on both of my Linux
laptops. https://github.com/hecrj/iced/issues/212. This is a deal-breaker -- I
want stuff I make to run anywhere without hassle. I looked at adding a glium
backend to iced, but the current structure has tons of backend-specific code. In
contrast, ezgui has a super tiny backend API -- basically just upload triangles
and draw them -- and all of the heavy logic sits on top of that.

## Naming

- coldbrew (stronger than iced coffee? ;) )
- allegro (where most pivotal meetings with my UX designer have happened, but
  this is also the name of some library)
- coco (for the geom library)
- abstgui (too obvious)
