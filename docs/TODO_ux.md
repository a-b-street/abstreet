# TODO - GUI and UX

## Quick n easy

- audit all panics
- tune text color, size, padding
- sort the top menus

- click cycle diagram to edit duration
- lane edit validity
- make it easy to see current lane when changing it

## Less easy

- revamp stop sign editor
- toggle rewind mode
	- sim stuff feels like a modal menu that's kinda omniprescent, but gets hidden sometimes

## General ezgui stuff

- trigger screencap from a top menu debug thing WITHOUT a hotkey.
- optionally limit canvas scrolling/zooming to some map bounds
- T top menu doesnt know when we have a more urgent input thing going!
- cant use G for geom debug mode and contextual polygon debug
- on a menu with preselected thing, clicking ANYWHERE does stuff...
- X on all menus

## New features

- swap direction of one-way
- convert between one- and two-way if there's enough space

- undo support for edits

## Better rendering

- render overlapping peds reasonably
- draw moving / blocked colors (gradually more red as they wait longer)
- render cars with textures?
- rooftops
	- https://thumbs.dreamstime.com/b/top-view-city-street-asphalt-transport-people-walking-down-sidewalk-intersecting-road-pedestrian-81034411.jpg
	- https://thumbs.dreamstime.com/z/top-view-city-seamless-pattern-streets-roads-houses-cars-68652655.jpg
- https://gifer.com/en/2svr
- color tuning
	- neutral (white or offwhite) color and make noncritical info close to
	  that. http://davidjohnstone.net/pages/lch-lab-colour-gradient-picker,
          chroma < 50
