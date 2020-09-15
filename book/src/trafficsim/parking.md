# Parking

TODO: Fill out the types of parking available, public/private, blackholes, how
people pick spots, how seeding works, etc.

## Infinite parking

If you pass `--infinite_parking` on the command line, every building gets
unlimited public spots. This effectively removes the effects of parking from the
model, since driving trips can always begin or end at their precise building
(except for blackhole cases). This is useful if a particular map has poor
parking data and you need to get comparative results about speeding up some
trips. Often the A/B testing is extremely sensitive, because a parking space
close to someone's destination is filled up quickly, slowing down the trip.
