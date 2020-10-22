# Live edits

When the player edits the map, there's an [efficient process](../map/edits.md)
for applying the edits at the map model and rendering layer. In the middle of a
simulation, it's less obvious how to apply all edits. Most of the time
currently, edits cause the simulation to reset to midnight. Applying edits to
the sim without reset is important for running machine learning experiments and
for improving the gameplay experience (by having more immediate feedback about
the consequences of a change).

The UI has a `dirty_from_edits` bit to track when changes have been applied
without reset. This lets us tell the player that by the end of the day, any
score / results are tentative, because their edits might have a different effect
earlier in the day.

## What works today

Changes to traffic signals are simple -- `incremental_edit_traffic_signal`
happens at the map layer, and then `handle_live_edited_traffic_signals` at the
sim layer just resets the current stage to 0 if the previous configuration had
more stages.

## TODO: Recalculating paths

Many of the edits will influence routes. For trips that haven't started yet,
there's nothing to do immediately. Paths are calculated right before the trip
starts, so slight changes to the start/end of the path due to map edits (like
where somebody starts biking, for example) are captured naturally.

For currently active trips, in some cases, rerouting would be ideal but not
necessary (like if speed limits changed). In other cases -- like changing access
restrictions, modifying lane types, closing intersections -- the route must be
recomputed. As a simple first attempt, we could just cancel all active trips
whose path crosses an edited road or intersection. Later, we can figure out
rerouting.

And actually, the only other case to handle is `ChangeRouteSchedule`, which
should just be rescheduling the `StartBus` commands.

## TODO: Parking

What happens if you modify a parking lane while there are cars on it? For now,
just delete them. Trips later making use of them will just act as if the car
never had room to be spawned at all and get cancelled or fallback to walking.

A better resolution would be to relocate them to other parking spots. If the
owner is home, it'd be neat to have them walk outside, move the car, and go back
in. But this greatly complicates the simulation -- the edited lane is in a
transition state for a while, it modifies schedules, the person might not be
around, etc.
