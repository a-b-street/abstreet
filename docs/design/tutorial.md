# Tutorial mode

## Synthetic maps

For tests and tutorial mode, I totally need the ability to create little
synthetic maps in a UI. Should be different than the main UI.

What are the 'abstract' objects to manipulate?

- Intersections... just points
	- Move these, have the roads move too
- Ability to connect two intersections with a straight line road
	- Edit lane type list in each direction
	- This lets border nodes be created
- Place rectangular buildings

This should basically use raw_data as primitives... or actually, no. GPS would
be weird to work with, and roads should be expressed as the two intersections,
so we don't have to update coordinates when we move intersections.

How to map lanes to stuff that make/lanes.rs will like? Might actually be easy,
actually.

Ideally, would render the abstract thing in one pane, and live-convert to the
full map and display it with the editor code in the other pane. But as the
halloween experiment shows -- that'd require a fair bit more refactoring first.
