## Gameplay

Most players will repeatedly:

1) Start in sandbox mode, watching a traffic simulation and browsing around for problems.

2) Use the map edit mode to change lane types and adjust intersections to try
to fix some particular problem.

3) Run an A/B test to compare how the same trips perform with and without some edits.

## Parking

On-street parking lanes are modeled, with the available spots based on the
road's length. The blockface dataset from King County GIS is used to infer
which roads have a parking lane -- but this dataset isn't meant to be so
accurate, so the results are often incorrect.

Driving trips between buildings (not starting or ending outside the map) are
multi-modal: the trip starts with a pedestrian leaving a building by foot,
walking to the closest parked car that they own, driving to their destination,
and then wandering around to look for a free parking spot (which can take a
while if there are no free parking spots nearby!). Simulations start with each
building having some number of parked cars spawned somewhere close by.

gif of ped entering a car
gif of ped parking a car and going to a bldg

TODO:
- More accurate data for the number of cars associated with each household
- Distinguish between types of on-street parking (free, pay by the hour, residences-only restrictions)
- Off-street parking (driveways, public and private parking lots and garages)

## Trips

During a typical day in Seattle, where do people travel, when do they depart,
and do they walk, bike, bus, or drive there? A/B Street imports trip data from
PSRC's [Soundcast](https://www.psrc.org/activity-based-travel-model-soundcast),
which models a synthetic population and has been carefully calibrated to match
census data, travel surveys, landuse, vehicle counts, and so on.

Mission Edit mode contains tools to visualize individual and aggregate trips
from this data, without running a traffic simulation.

gif of these tools

TODO:
- Trips that begin and end outside the boundary of a map, but pass through it,
  are currently skipped. This happens often for trips passing through I5 or
  520.

## Map borders


## Buildings

home / commerical / mixed-use, etc. number of households and employees from psrc.

## Generalizing to other cities

anywhere with OSM data... reasonable defaults
