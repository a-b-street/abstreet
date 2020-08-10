# Year 2 (June 2019-2020)

![Circa October 2019](oct_2019.png)

- June: contraction hierarchies for pathfinding, stackable game states

- July: OSM turn restrictions, misc (I think I was in Europe?)
- August: pedestrian crowds, agent color schemes, parking blackholes, a big
  `raw_data` refactor to store `Pt2D`, attended first hackathon
- September: offstreet parking, associating parked cars with buildings using
  Soundcast (before that, anybody could use any car!), implemented texture
  support for some reason, doing manual `MapFixes` at scale to fix OSM bugs

  - **milestone**: got the smallest montlake map to run without gridlock

- October: parking sim fixes, opportunistic lane-changing, starting challenge
  modes
- November: prebaked sim results, time-series plots, undo for edit mode, traffic
  signal editor grouping turns
  - **milestone**: Yuwen joins project
- December: the UI reform begins (flexbox, minimap, trip timelines, cutting over
  to SVGs, info panels, scrolling), started naming releases sensibly

  - Project leaked to [HN](https://news.ycombinator.com/item?id=21763636), woops

- January: UI reform continues, the modern tutorial mode appears
- Feburary: UI and tutorial, all text now pure vectors, port to glow+WASM
- March: lockdowns start in US, start grouping trips as a person, population
  heatmap, left-hand driving, info panel and typography overhauls. started
  engaging with Greenways, started effort to map traffic signals

- April: Orestis joins and starts the pandemic model, trip tables, the optimize
  commute challenge, refactor for people's schedules and owned vehicles, trip
  time dat viz, MAJOR progress fixing gridlock at the sim layer
- May: gridlock progress, upstreaming fixes in OSM, differential throughput and
  first real write-up, long-lasting player edits, dedicated parking mapper,
  maybe vanquished the HiDPI bugs, multi-step turn restrictions, random bios for
  people, and docs like this to prep for launch ;)
  - **milestone**: relying on pure OSM, no more `MapFixes`
