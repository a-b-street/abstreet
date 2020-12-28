# Gridlock

Here "gridlock" refers to the general problem of trips getting permanently
stuck, preventing the full simulation from completing. Most of the work is
tracked [here](https://github.com/dabreegster/abstreet/issues/114).

The general lesson is: you can't code your way around all edge cases. The data
in OSM often needs manual fixes. It's often useful to spend coding effort on
tools to detect and fix OSM problems.

## Problems

The choices in the movement model matter. Some gridlock is inherent to any
system with queueing and conflicting turns. But in reality, people wiggle around
partly blocked turns. And some of this comes from the treatment of the
front/back of vehicles.

- Short roads in OSM causing very weird geometry
- Intersection geometry being too large, requiring too much time to cross
- Unrealistic traffic patterns caused by everyone trying to park in one big
  garage (downtown) or take some alley (the UW soundcast issue)
- Too many people try to take an unprotected left turn (often at a stop sign)
- Bad individual traffic signals, usually at 5- or 6-ways
- Groups of traffic signals logically acting as a single intersection
- Separate traffic signals along a corridor being unsynchronized
- Vehicles performing illegal sequences of turns
- Vehicles are stuck with their plan and not reacting to traffic by changing
  route
- Real traffic would result in a gridlock without a deliberate actions to avoid
  it. Such actions range from individual decisions of drivers to police manually
  controlling traffic. Intelligent avoidance of gridlock is not simulated and is
  extremely hard to simulate.
- Vehicles will wait in lane filled with already waiting vehicles, even if there
  is a completely empty lane allowing travel in desired direction. It makes
  easier for entire lane between crossings to fill, contributing to gridlocks.
  Note that while this and other clearly stupid behaviors are clearly
  unrealistic, it is not trivial to implement more realistic and more efficient
  decisions.
- Issues caused by the unrealistic
  [lane-changing model](discrete_event.md#lane-changing)
  - Two turns that go to the same lane (one going "straight", the other often a
    lane-change) conflict. The conflict is coarse, at the granularity of the
    entire intersection. So if vehicles are piled up in two lanes trying to
    merge into one, then one group is likely to go through as expected, but the
    second group will wait for the first to completely clear the intersection.
    Until then, it looks like a conflicting turn is being done.

## Solutions

Divide into implemented or not.

- Synchronizing pairs of signals
- Uber-turns
  - for interpreting OSM turn restrictions
  - for synchronizing a group of signals
  - for locking turn sequences
    - Once a vehicle starts an uber-turn, prevent others from starting
      conflicting turns on nearby intersections. Until groups of traffic signals
      are configured as one, this is necessary to prevent somebody from making
      it halfway through a sequence then getting blocked.
- Cycle detector
- block-the-box protection
  - the manual list of overrides
  - likely shouldn't apply during uber-turns
  - is it always fine to block the box at degenerate intersections?
- hacks to allow conflicting turns at really broken intersections
- manually timing signals
- penalties for lane choice to make lane usage realistic

### Not implemented

- Dynamic rerouting
- Allow multiple vehicles through intersection at once if there is enough space
  on lane where given vehicle is going. Currrently vehicles travel through
  crossings one by one (or, with `--disable_block_the_box` enabled - will enter
  crossing even if leaving it will be impossible).
- Last resort: if someone's waiting on a turn >5m, just go.
- Uber-turns
  - Group both stop sign and traffic signal intersections when looking for
    uber-turns. Even a single traffic signal surrounded by tiny roads with stop
    signs is causing problems.

## Strategy for resolving

Because there are so many different causes all tangled together, my approach is
to simplify the simulation as much as possible. A problem is much easier to
understand and fix when it's isolated. I've been trying this to get the downtown
weekday scenario to complete. A list of different techniques to simplify, in no
particular order:

- Use the `--infinite_parking` flag to just let everyone park directly in their
  destination buildings. This is useful since downtown has many large parking
  garages with high capacity, but I don't have a data source describing them.
- Use the `--disable_turn_conflicts` flag, which greatly reduces realism, but
  lets conflicting turns happen simultaneously. (Even with this and other flags,
  downtown still gridlocks!) It also disables traffic signals, so bad inferred
  timing isn't an issue.
- Use the `--disable_block_the_box` flag to workaround short roads.
- If you notice problems forming from cars stacking up behind slower cyclists,
  there's no over-taking implemented yet. Use the scenario modifiers to convert
  all biking trip to driving:
  `--scenario_modifiers='[{"ChangeMode":{"to_mode":"Drive","pct_ppl":100,"departure_filter":[0.0,86400.0],"from_modes":["Bike"]}}]'`
- If all else fails, use the scenario modifiers to bluntly cancel some
  percentage of all trips.

A quick method for "disabling" buggy short roads is to use edit mode and close
the lanes. This forces traffic to reroute around the problematic area, which
might create other problems, but it can be useful.

## Fixing data used in simulation

Give more examples of changesets.

- upstreaming turn restrictions into OSM to prevent invalid U-turns and other
  crazy movements
  - ex: <https://www.openstreetmap.org/changeset/87945050>
- upstreaming lane count fixes into OSM to improve geometry
