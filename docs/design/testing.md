# Testing-related design notes

## Simulation unit tests

To encourage testing, it should be easy to:
	- describe a setup
	- assert what the outcome should be
		- sometimes just that no runtime invariants are broken
	- pop up a UI to interactively step through the test

Some first tests to write:
	= car starting with no path on road with no parking spots, ensure they wind up parking at the first spot on some
	side street            
	- car stops for departing car (winds up following it)
	- departing car waits for other car (winds up following it)
	- a line of cars moving through a stop sign looks jittery right now. correct or not?
	- following distances for cars of different lengths

Unclear how to nicely let the test inspect stuff every tick.

Rejected ideas:
- make every pub(crate) so a unit test can reach into state anywhere. It ruins viz for testing.
- JSONify stuff and look at that. too slow, and walking the JSON structure is annoying and not type-safe.
- make one-off accessors for interesting stuff. pollutes code and is tedious.

The idea that's sticking:
- every tick, accumulate a list of events that occurred. publish these from various places.
	- most of the events are state transitions -- car leaves lane, intersection accepts ticket, car parks, bus departs
	- beyond unit testing, this will be useful for building up a compressed schedule for the time traveler
	- and already am kind of using this pattern to communicate between sim managers, spawners, etc
	- will help compute trip statistics later
	- would also be nice to log some of these

## Watch tests easily

- need to organize savestate captures
	- dedicated place: data/savestates/MAP/scenario/time
		- plumb map name, scenario name
		- should be able to just point to one of these saves, not refer to the map or RNG seed again
	- also kinda needed for time traveling later

- when a problem happens, we want to back up a little bit
	- probably just need automatic occasional savestating, and to print a nice command to rerun from it

## Diffing for A/B tests

Basic problem: how do we show map edits/diffs?
	- could be useful for debugging as new data sources come in
	- and is vital part of the game
	- UI
		- highlight edited things
		- hold a button to show the original versions of things in a transparentish overlay

How to show diffs for agents?

## Test setup

Tried having some helper methods, but they're not aging well as the new trip
leg stuff continues. Removing for now, will re-evaluate later.

Synthetic editor is awesome. How could it help test setup?

- could specify things like parking spots directly
	- but that may be lots of UI work
	- and then a synthetic map is only useful for one test, maybe
- could just label things in the synthetic editor
	- and easily look em up in the test setup code

## Test runner

- want to tag tests and run groups of them easily
	- have to use prefixes to test names for now
- want nicer runner output
	- capture output for failed tests in files
	- print summary of each test at the end
	- for sim tests, print a command to run it in the UI

Let's experiment and make a new binary crate for all tests!

- it'd be neat to later integrate with travis.
