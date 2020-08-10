# Year 1 (June 2018-2019)

I skimmed through git and summarized roughly what I was working on each month,
calling out milestones. "UI churn" is pretty much constantly happening.

- June: polyline geometry and lanes, building paths, protobuf -> serde

- July: pedestrians, bikes, parked cars, lane edits
- August: porting AORTA's discrete-time driving model
- September: multi-leg trips, buses, the first ezgui wizard, randomized
  scenarios

- October: A/B test mode (and so per-map plugins), forking RNG for
  edit-invariance, intersection geometry
- November: clipping / borders, using blockface for parking, time travel mode,
  test runner framework
- December: bezier curves for turns, traffic signal editor, a first attempt at
  merging intersections, right-click menus, a top menu, modal menus

  - the grand colorscheme refactor: a python script scraped `cs.get_def` calls
    at build-time

- January: careful f64 resolution, ezgui screencapping, synthetic map editor
  - **grand refactor**: piston to glium
- February: attempting to use time-space intervals for a new driving model, new
  discrete-event model instead
  - **Feb 19-27**: conceiving and cutting over to the new discrete event model
- March: fleshing out DES model (laggy heads), first attempt to build on
  windows, gridlock detection

- April: first public releases, splash screen and rearranging game modes
- May: fancier agent rendering, attempting to use census tracts, finding real
  demand data
  - **milestone**: discovered PSRC Soundcast data, much more realistic trips
