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

## Bike/bus lane connectivity

Bikes and buses can make crazy left turns from the rightmost protected lane.
Alternatively, stop generating those turns and start generating turns between
protected and general lanes.

## Parking

No restrictions -- all available spots are treated equally. No cost, time
limits, or private spots.

## U-turns

Only happen at dead-ends right now. But there are a few important ones to
support -- like Montlake Blvd to go WB on 520.

## Roads without sidewalks

Last-leg routing... pedestrians need to walk on the road. How to model this?
Happens in Seattle when there's parking without sidewalks nearby.

## One-at-a-time roads

Some roads are "two-way", but have parking on both sides, and so are effectively
one way at a time. How's this tagged in OSM? How do we model this?
