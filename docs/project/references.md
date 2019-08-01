# References

## Example use cases

- https://www.reddit.com/r/SeattleWA/comments/9mtgkh/seven_places_to_add_bus_lanes_now/
- https://www.reddit.com/r/SeattleWA/comments/9oqkz9/how_traffic_patterns_will_change_after_seattles/
- https://www.reddit.com/r/Seattle/comments/9orqne/4_fresh_ideas_to_ease_seattles_coming_traffic/

## Groups that may be eventually interested

- Seattle Times Traffic Lab
- https://www.citylab.com/transportation/2018/08/is-it-time-to-rethink-what-a-bike-lane-is/568483/
- http://openseattle.org/
- https://igniteseattle.com/
- http://seattlegreenways.org/
- https://www.livablecities.org/
- https://www.reddit.com/r/openstreetmap/comments/a39uv0/ok_so/
- https://mic.comotion.uw.edu/
- https://www.seattleinprogress.com/
- http://www.seattle.gov/seattle-pedestrian-advisory-board
- Socrata
- http://transportationcamp.org/
- https://www.seattle.gov/transportation/projects-and-programs/programs/neighborhood-street-fund
  /
  https://www.seattle.gov/neighborhoods/programs-and-services/your-voice-your-choice
- https://commuteseattle.com/
- https://www.theurbanist.org/
- https://humantransit.org/2019/03/notes-on-simcity-at-30.html
- https://mynorthwest.com/category/chokepoints/
- https://blogs.uw.edu/ceadvice/2019/05/08/infrastructure-week-2019-welcome-uw-cee-students-and-faculty/
- https://escience.washington.edu/dssg/
- josie kresner from transport foundry

## Similar projects

- Urban Footprint (https://news.ycombinator.com/item?id=17895739)

## Seattle-specific

SDOT asking for feedback:

- http://sdotblog.seattle.gov/2017/02/08/from-signals-to-signs/
- https://www.seattle.gov/transportation/projects-and-programs/programs/bike-program/protected-bike-lanes/n-34th-st-mobility-improvements
- https://www.seattle.gov/transportation/projects-and-programs/programs/transportation-planning/north-downtown-mobility-action-plan
- https://www.seattlebikeblog.com/2016/12/01/check-out-seattles-12-winning-neighborhood-led-transportation-ideas/

Seattlites with opinions and ideas:

- http://seattlegreenways.org/
- https://www.seattlebikeblog.com/2018/01/19/a-roosevelt-junior-redesigned-the-streets-around-his-high-school-and-his-plan-is-better-than-sdots/
- https://www.reddit.com/r/SeattleWA/comments/5rvss5/what_changes_would_you_make_to_seattles_bus/
- https://www.seattletimes.com/seattle-news/transportation/congestion-tolling-could-finally-break-seattles-working-poor-heres-a-better-idea/
- https://www.reddit.com/r/SeattleWA/comments/86g3p9/id_get_back_an_hour_and_a_half_a_week/
- https://www.reddit.com/r/Seattle/comments/4z3ewl/what_are_seattles_worst_intersections/
- https://www.reddit.com/r/SeattleWA/comments/83h4ri/the_intersection_at_john_and_broadway_desperately/

- http://www.seattle.gov/transportation/sdot-document-library/citywide-plans/move-seattle

## Other projects

- https://github.com/uwescience/TrafficCruising-DSSG2017
- http://sharedstreets.io/
- https://github.com/twpol/osm-tiles attempting to infer nice road geometry too

## Notes from related work

### SMARTS (https://people.eng.unimelb.edu.au/etanin/tist17.pdf)

- Split map into sections, simulate in parallel, load-balance
- has an IDM equation
- tests against real TomTom data of average speed per link

### Games

SimCity, Cities: Skylines
https://steamcommunity.com/sharedfiles/filedetails/?id=583429740
https://github.com/fegennari/3DWorld

### Open source urban planning

UrbanSim

### Proprietary

Sidewalk Labs Model

### Maps for people

https://arxiv.org/pdf/1811.01147.pdf

### gamma.cs.unc.edu/RoadNetwork/wilkie_TVCG.pdf

section 6.3 talks about offset polylines http://gamma.cs.unc.edu/RoadNetwork

### CityBound

https://github.com/aeickhoff/descartes

### Discrete Event Simulation papers

- section 5.1 of Advanced tutorial on microscopic discrete-event traffic
  simulation refers to some DES systems

  - Florian, Mahut, and Tremblay 2008
  - Sumaryo, Halim, and Ramli 2013
  - Salimifard and Ansari 2013
  - Burghout, Koutsopoulos, and Andreasson 2006
  - Thulasidasan, Kasiviswanathan, Eidenbenz, Galli, Mniszewski, and Romero 2009

- A Dynamic Traffic Assignment Model for Highly Congested Urban Networks
  - section 2.2 models lanes as a moving and queueing part, references other
    possibly useful papers
  - dont worry about multiple lanes for the moving part, just the turn queues at
    the end

## Tactical urbanism

- https://www.vice.com/en_us/article/pajgyz/rogue-coder-turned-a-parking-spot-into-a-coworking-space
