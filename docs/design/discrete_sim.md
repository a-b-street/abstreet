# Discrete simulations

## Why are timesteps bad?

Discrete-time sim has two main problems:

1) It's fundamentally slow; there's lots of busy work.

2) Figuring out acceleration in order to do something for the next tick is complicated.

Discrete-event sim is the real dream. I know when a ped will reach the end of a
sidewalk and can cheaply interpolate between for drawing. If I could do the
same for all agents and states/actions, I could switch to discrete-event sim
and SUPER speed up all of the everything.

This possibly ties into the FSM revamp idea perfectly. Every state/action has
two durations associated with it:

- the best case time, calculated while pathfinding, which assumes no
  conflicts/delays due to scarcity
- the actual time something will take, based on state not known till execution
  time

## Intent-based kinematics

Instead of picking acceleration in order to achieve some goal, just state the
goals and magically set the distance/speed based on that. As long as it's
physically possible, why go through extra work?

Lookahead constraints:
- go the speed limit
- dont hit lead vehicle (dist is limited by their dist + length + following dist)
- stop by a certain point

## The software engineering question

Is there a reasonable way to maintain both models and use one headless/editor
for them? Seemingly, the entire Sim API needs to just be abstracted. Like the
DrawAgents trait, but a bit more expanded. Except stuff like time travel
applied to discrete-time sim can't support debugging agents or showing their
path. I think time travel with discrete-event would work fine -- just storing
all the previous states with the exact times.
