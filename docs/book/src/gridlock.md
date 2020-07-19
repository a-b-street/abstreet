# Gridlock

Here "gridlock" refers to the general problem of trips getting permanently
stuck, preventing the full simulation from completing. Most of the work is
tracked [here](https://github.com/dabreegster/abstreet/issues/114).

My general approach right now to get a map working is to cancel some percent of
all trips, look for individual problems, fix them, and repeat. Once the full day
works, cancel less trips. It's easier to isolate the root cause of problems when
there are less of them erupting simultaneously.

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
- Vehicles are stuck with their plan and not reacting to traffic by changing route
- Real traffic would result in a gridlock without a deliberate actions to avoid
  it. Such actions range from individual decisions of drivers to police manually
  controlling traffic. Intelligent avoidance of gridlock is not simulated and
  is extremely hard to simulate.
- Vehicles will wait in lane filled with already waiting vehicles, even if there
  is a completely empty lane allowing travel in desired direction. It makes
  easier for entire lane between crossings to fill, contributing to gridlocks.
  Note that while this and other clearly stupid behaviors are clearly unrealistic,
  it is not trivial to implement more realistic and more efficient decisions.

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
  - Group both stop sign and traffic signal intersections when looking for
    uber-turns. Even a single traffic signal surrounded by tiny roads with stop
    signs is causing problems.
- Cycle detector
- block-the-box protection
  - the manual list of overrides
  - likely shouldn't apply during uber-turns
  - is it always fine to block the box at degenerate intersections?
- hacks to allow conflicting turns at really broken intersections
- manually timing signals
- Last resort: if someone's waiting on a turn >5m, just go.
- Dynamic rerouting

### Fixing data used in simulation

- upstreaming turn restrictions into OSM to prevent invalid U-turns and other
  crazy movements
- upstreaming lane count fixes into OSM to improve geometry
