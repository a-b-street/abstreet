# Time travel

aka reversible simulation. There are two ways to think about implementing this:

- the full thing -- make the entire Sim rewind
- just a UI trick
	- conceptually, be able to reproduce the Draw{Car,Ped}Inputs parametrically
	- baseline: literally record those inputs every tick and just lookup. huge memory hog.
	- the cooler thing: record timelines of agents as the real sim plays and interpolate.

Going to just focus on the UI trick approach for now. To be useful, what things in the UI need to work?
- rendering of course -- show things where they were
- warping to an agent? easy
- showing route of an agent? hard

So we need to disable some plugins while we're in this special mode. I wonder
if each plugin should sort of declare a dependency on a live sim or if at a
higher level, the list of plugins should change?

Some initial steps:
= make a plugin that asks for all Draw stuff every tick and just saves it
- activate the time travel plugin and have keys to go back/forward
- supply the Draw{Car,Ped} stuff from the time travel plugin, not the sim
- deactivate lots of other plugins while in this mode
	- make sim ctrl a proper plugin
