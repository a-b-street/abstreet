# A/B Street Instructions

General disclaimer: This is a very rough demo. The user interface is clunky, and
gameplay is not cohesively tied together yet. Please email
<dabreegster@gmail.com> or
[file a Github issue](https://github.com/dabreegster/abstreet/issues/) if you
hit problems.

## Installing the game

Grab a pre-built binary release -- updated every Sunday, announced at
[r/abstreet](http://old.reddit.com/r/abstreet):

- Windows:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.22/abstreet_windows_v0_1_22.zip
- Mac:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.22/abstreet_mac_v0_1_22.zip
  - The minimap may be missing, depending on your monitor's DPI. If so, modify
    the `play_abstreet.sh` script to pass `--hidpi_factor=2.0` and experiment
    with different values. If you have to do this, please
    [file an issue](https://github.com/dabreegster/abstreet/issues) and let me
    know, so I can figure out why `glutin` misreports this.
- Linux:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.22/abstreet_linux_v0_1_22.zip

Unzip the folder, then run `play_abstreet.sh` or `play_abstreet.bat`. On
Windows, you'll probably get a warning about running software from an unknown
publisher.

Or you can [build from source](/docs/dev.md).

## Playing the game

General controls:

- Click and drag to move
- Scroll wheel or touchpad to zoom
- Menus should work as expected. Most actions have a keybinding.
- You can hover over an object and left-click to see more actions.

Things to try:

- In sandbox mode, hover over an intersection, right click, and spawn agents.
- To run a realistic, full day's worth of traffic, go to sandbox mode, then
  "start a scenario" (hotkey **s**) and choose the "weekday_typical_traffic"
  entry. Time (shown in the top-right corner) starts at midnight. Things tend to
  get interesting around 6am -- use the speed controls in the top-left. Try
  zooming in for details, and zooming out to see an overview.
  - The full scenario should complete successfully on the small montlake map. On
    other maps, it currently gridlocks or crashes. Most of these are known
    issues.
- Go to edit mode (note this will reset the simulation). Use the buttons at the
  top to pick a lane type, then hover over a lane and press **space** to change
  it. Deselect the lane tool, then click an intersection to edit stop signs or
  traffic signals.
- Go back to the main menu and pick a challenge. Eventually these will be very
  self-guided, but there are various problems with all of them right now.

## Data source licensing

A/B Street binary releases contain pre-built maps that combine data from:

- OpenStreetMap (https://www.openstreetmap.org/copyright)
- King County metro
  (https://www.kingcounty.gov/depts/transportation/metro/travel-options/bus/app-center/terms-of-use.aspx)
- City of Seattle GIS program
  (https://www.opendatacommons.org/licenses/pddl/1.0/)
- https://github.com/seattleio/seattle-boundaries-data
  (https://creativecommons.org/publicdomain/zero/1.0/)
- Puget Sound Regional Council
  (https://www.psrc.org/activity-based-travel-model-soundcast)

Other binary data bundled in:

- DejaVuSans.ttf (https://dejavu-fonts.github.io/License.html)
- Roboto-Regular.ttf (https://fonts.google.com/specimen/Roboto, Apache license)
