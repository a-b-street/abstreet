# Modeling assumptions

This is pretty disorganized right now, just need to start something.

## Sidewalk connectivity

Should it be possible to close sidewalks for construction? Right now, this
breaks too many assumptions that're hard to recompute. Building front paths and
bus stops are associated with a sidewalk, so that makes applying the edit way
more unclear. Closing intersections is still useful -- remove all of the vehicle
turns, but allow the walking turns.

## Graph connectivity

For now, no map edits should be able to impact any of the trips possible in the
baseline -- so no impacting graph connectivity, no killing bus stops, etc.

## Over-taking

Unsupported right now, but it should happen. Unlocks shared bike/ped trails like
the Burke.

## Left turns in the middle of a road

Into a driveway, or using the shared left turn lanes. This should be supported.
Parking and unparking already have the ability to block one queue -- extend
that.

## Demand modeling

When the player makes it much more/less convenient to take some trip, people
will eventually shift mode or take different trips altogether. Not attempting
any of that yet -- just using PSRC trips. I don't understand the demand modeling
process well at all yet.

## Bikes using bus lanes

This should be pretty easy to allow.
