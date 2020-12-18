# Roadmap

A/B Street has been under active development since June 2018. That's a long time
-- what work is happening now and how can you contribute?

## Next steps, summer 2020

Afer the alpha launch in June, I plan to focus on:

- shared biking/walking trails like the Burke Gilman
- light rail
- more score functions besides trip time, like safety/comfort
- changing trip mode choice (if you make a bus route more desirable, switch some
  trips)
- web support (so people can try out proposals without installing anything)

## Ongoing work

If I had resources to hire a team, this is roughly how I'd organize different
roles. If you're interested in helping, these aren't strictly defined positions,
just ideas of related tasks.

### UI and data visualization

We've got a UX designer, but implementing all of the new designs takes time.
Also:

- improve color schemes for colorblind players, implement night mode, rain
  effects, etc
- refactor and clean up the GUI library for other Rust users
- lots of data viz design / implementation needed

### Game design

- the tutorial mode needs attention
- many ideas for challenge/story modes, but playtesting, tuning, and game design
  needed

### Map data / GIS

Support more cities:

- write docs/tools to help people add new cities without programming experience
- add support for non-OpenStreetMap input: GeoJSON for parking in Perth, other
  trip demand sources, etc
- fix bugs for driving on the left side of the road

Improve the quality of map geometry derived from OpenStreetMap:

- try new algorithms to generate intersection polygons
- make tools for easily improving relevant data in OSM
- use ML and lidar/satellite data to get extremely accurate curb / planter /
  sidewalk geometry

Build tools and organize community mapping:

- organize an effort to map how traffic signals are timed (partly started)
- divide and track work for distributed mapathons

Bring in new data to understand more about cities:

- PM2.5 pollution
- Tax / land value (is there inequitable access to transit?)

### Simulation / modeling

Totally new areas:

- light rail
- shared bike/pedestrian paths
- ridesharing
- micromobility (scooters, floating bikeshare)
- more score functions (elevation gain, biking safety)
- generating trip demand / activity models from scratch or modifying existing
  ones

Improve existing models:

- overtaking / lane-changing
- pedestrian crowds
- instant vehicle acceleration
- pedestrians walking on road shoulders (some streets have no sidewalks)
- buses: transfers, proper schedules, multiple buses per route

### Web

A/B Street runs on the web via WASM and WebGL; just waiting on vector text
support. Besides that:

- Share community proposals online, discuss them, vote, etc

## Contributing for non-programmers

There's plenty to do besides programming!

- Mapping, most of which directly contributes to OpenStreetMap:
  - sidewalks and crosswalks
  - [on-street parking](../side_projects/parking_mapper.md)
  - [traffic signal timing](https://docs.google.com/document/d/1Od_7WvBVYsvpY4etRI0sKmYmZnwXMAXcJxVmm8Iwdcg/edit?usp=sharing)
- Playtesting by attempting to implement real proposals would also be helpful,
  to expose where it's awkward for A/B Street to edit the map and to write up
  problems encountered.
- Advocacy: I'm not great at finding the right people to to get ideas
  implemented for real. Maybe you are?

## Long-term vision

Longer term, I'd like to take lots of the work in generating and interacting
with high-detail OpenStreetMap-based maps and generalize it, possibly as a new
OSM viewer/editor.

More generally, I'd like to see how simulation can help individuals understand
and explore other policy decisions related to cities. Domains I'm vaguely
interested in, but not at all knowledgable about, include land-use / zoning,
housing, and supply chains. In late March 2020, a new collaborator started a
pandemic model using the existing simulation of people occupying shared spaces.
What are other domains could benefit from the rich agent-based model we're
building?
