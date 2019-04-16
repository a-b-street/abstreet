# A/B Street Instructions

General disclaimer: This is a very rough demo. The user interface and controls
are horrible. Don't expect things to work well, or at all. Please email
`dabreegster@gmail.com` or file a Github issue if you hit problems.

## Installing the game

...

## Running the game

Start the game by running `run_montlake.sh` or `run_montlake.bat`. On Windows,
you'll probably get a warning about running software from an unknown publisher.
Two maps are included in this release -- `montlake` is a small slice around the
Montlake neighborhood, `23rd` is a larger slice around 23rd Ave.

General controls:

- Click and drag to move
- Scroll wheel or touchpad to zoom
- Hover over something and hold Control to examine it
- Select something and right click for a menu of relevant actions. The keyboard
  shortcuts also work just by selecting that object.
- Explore the menu bar at the top.

Simulating traffic:

- Choose `seed the sim with agents` from the `Simulation` menu, or just press
  `s` while nothing is selected. This will spawn a bunch of parked cars, buses,
  and pedestrians leaving buildings. Some of the pedestrians will get into a
  parked car and start driving somewhere.
- Press `space` to pause/resume the simulation. `]` speeds things up, and `[`
  slows things down. You can find all of these in the `Simulation` menu.
- Spawn a single agent by selecting the starting building and pressing `F3` (or
  right clicking the building and using the menu). Select the goal building and
  confirm with `F3`. You can hover over any agent to see its route; press `r`
  (or right-click the agent) to keep drawing the route.
- Hover over an intersection and press `z` to spawn a bunch of cars nearby.

Editing the map:

- You can only make edits when a simulation isn't running. If you already
  started one, quit and start over.
- Select an intersection and press `e`. You can then select an individual turn
  icon and press `space` to change its priority.
- Enable `edit roads` from the `Edit` menu, select a lane, and press `space` to
  change its type.
- Save your changes using `manage map edits` in the `Edit` menu.

## Data source licensing

A/B Street binary releases contain pre-built maps that combine data from:

- OpenStreetMap (https://www.openstreetmap.org/copyright)
- King County metro
  (https://www.kingcounty.gov/depts/transportation/metro/travel-options/bus/app-center/terms-of-use.aspx)
- City of Seattle GIS program
  (https://www.opendatacommons.org/licenses/pddl/1.0/)
- https://github.com/seattleio/seattle-boundaries-data
  (https://creativecommons.org/publicdomain/zero/1.0/)
- USGS SRTM
- DejaVuSans.ttf (https://dejavu-fonts.github.io/License.html)
