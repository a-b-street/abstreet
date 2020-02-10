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
  https://github.com/dabreegster/abstreet/releases/download/v0.1.27/abstreet_windows_v0_1_27.zip
- Mac:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.27/abstreet_mac_v0_1_27.zip
- Linux:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.27/abstreet_linux_v0_1_27.zip

Unzip the folder, then run `play_abstreet.sh` or `play_abstreet.bat`. On
Windows, you'll probably get a warning about running software from an unknown
publisher.

Or you can [build from source](/docs/dev.md).

## Playing the game

General controls:

- Click and drag to move
- Scroll wheel or touchpad to zoom
- Click an object to see more actions
- The bottom bar shows key shortcuts

Things to try:

- The tutorial introduces many controls and mechanics.
- In sandbox mode, select an intersection, and spawn agents.
- Change the default "random" traffic in sandbox mode to "weekday" to see
  realistic trips over a full weekday.
  - The weekday traffic scenario should complete successfully on the small
    montlake map. On other maps, it currently gridlocks or crashes. Most of
    these are known issues.
- Go to edit mode (note this will reset the simulation). Select a lane type at
  the top, then hover over a lane and press **space** to change it. Deselect the
  lane tool, then click an intersection to edit stop signs or traffic signals.
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
