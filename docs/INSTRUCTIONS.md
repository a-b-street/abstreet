# A/B Street Instructions

General disclaimer: This is a very rough demo. The user interface is clunky, and
gameplay is not cohesively tied together yet. Please email
<dabreegster@gmail.com> or file a Github issue if you hit problems.

## Installing the game

The easiest method is to use pre-built binaries. Check
https://github.com/dabreegster/abstreet/releases for the latest version, though
I'll try to keep these links up-to-date:

- Linux:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.10/abstreet_linux.zip
- Windows:
  https://github.com/dabreegster/abstreet/releases/download/v0.1.10/abstreet_windows.zip
- Mac (seems to have a HiDPI bug, text is offset):
  https://github.com/dabreegster/abstreet/releases/download/v0.1.10/abstreet_mac.zip

### Compiling from source

To build, you need a Linux-like environment with `bash`, `wget`, `unzip`, etc.
You also `osmosis` for the import script. At runtime if you want to use the
screen-capture plugin, you need `scrot`.

1.  Install Rust, at least 1.38. https://www.rust-lang.org/tools/install

2.  Download the repository:
    `git clone https://github.com/dabreegster/abstreet.git`

3.  Download all input data and build maps. Compilation times will be very slow
    the first time.
    `cd abstreet; ./import.sh && ./precompute.sh --release --disable_psrc_scenarios`.
    Alternatively, you could seed the entire `data` directory from this
    [9/21/2019 copy](https://drive.google.com/open?id=1tpHuojh1e14ZQLBhjLWf_rB6dLKy-hV7).

If you build from source, you won't have the convenient launcher scripts
referenced below. Instead:

```
cd game
cargo run --release
```

## Running the game

Start the game by running `play_abstreet.sh` or `play_abstreet.bat`. On Windows,
you'll probably get a warning about running software from an unknown publisher.

General controls:

- Click and drag to move
- Scroll wheel or touchpad to zoom
- Follow on-screen controls otherwise. You can also try hovering over an object
  and right-clicking to see more actions. (These controls will show up more
  clearly soon.)

## Data source licensing

A/B Street binary releases contain pre-built maps that combine data from:

- OpenStreetMap (https://www.openstreetmap.org/copyright)
- King County metro
  (https://www.kingcounty.gov/depts/transportation/metro/travel-options/bus/app-center/terms-of-use.aspx)
- City of Seattle GIS program
  (https://www.opendatacommons.org/licenses/pddl/1.0/)
- https://github.com/seattleio/seattle-boundaries-data
  (https://creativecommons.org/publicdomain/zero/1.0/)
- DejaVuSans.ttf (https://dejavu-fonts.github.io/License.html)
- Puget Sound Regional Council
  (https://www.psrc.org/activity-based-travel-model-soundcast)
- http://www.textures4photoshop.com for textures
