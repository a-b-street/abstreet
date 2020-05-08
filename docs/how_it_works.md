# How A/B Street works

The big caveat: I'm a software engineer with no background in civil engineering.
A/B Street absolutely shouldn't replace other planning or analysis. It's just
meant to be an additional tool to quickly prototype ideas without expensive
software and formal training.

This page gives a non-technical overview. See
[here](https://github.com/dabreegster/abstreet/#documentation-for-developers)
for details.

## The map of Seattle

The map in A/B Street is built from
[OpenStreetMap](https://www.openstreetmap.org/about). You will notice many
places where the number of lanes is wrong; let me know about these, and we can
contribute the fix to OpenStreetMap. Many sidewalks and crosswalks are also
incorrectly placed.

People in A/B Street have to park their cars somewhere. I can't find good data
about either public or private parking. For now, I'm using a Seattle
[GeoData blockface dataset](http://data-seattlecitygis.opendata.arcgis.com/datasets/blockface)
to guess on-street parking, but this is frequently wrong. I'm assigning every
building one offstreet spot. This is wildly unrealistic, but I have nothing
better yet.

There's also no public data about how traffic signals in Seattle are timed. I'm
making automatic guesses, and attempting to manually survey as many signals
in-person as I can. I could really use help here!

## The traffic

Vehicles in A/B Street instantly accelerate and brake. They change lanes only at
intersections, and they can't over-take slower vehicles in the middle of a lane.
People walking on sidewalks can "ghost" through one another, or walk together in
a crowd -- before COVID-19, this was a reasonable model in most areas. Despite
these limits, I hope you'll find the large-scale traffic patterns that emerge
from the simulation to be at least a little familiar from your real-world
experiences.

People in A/B Street follow a specific schedule, taking trips between buildings
throughout the day. The trips come from
[PSRC's Soundcast model](https://www.psrc.org/activity-based-travel-model-soundcast),
which uses census, land-use, and vehicle count data to generate a "synthetic
population" roughly matching reality. The trip data is from 2014, which is quite
old.

When you make changes to the map in A/B Street, exactly the people still take
exactly the same trips, making the same decision whether to drive, walk, bike,
or take transit. Currently, your changes only influence their route and
experience along it.

## Missing things

Light rail, shared biking/walking trails like the Burke Gilman, and ridesharing
are some of the notable things missing right now.
