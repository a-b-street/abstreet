# A/B Street Instructions

This is an alpha-quality demo. Please email <dabreegster@gmail.com> or
[file a Github issue](https://github.com/dabreegster/abstreet/issues/) if you
hit problems.

## Installing the game

Grab a pre-built binary release -- updated every Sunday, announced at
[r/abstreet](http://old.reddit.com/r/abstreet):

- Windows:
  https://github.com/dabreegster/abstreet/releases/download/v0.2.8/abstreet_windows_v0_2_8.zip
  - Unzip the folder, then run `play_abstreet.bat`. You'll probably getting a
    warning about running software from an unknown publisher.
- Mac:
  https://github.com/dabreegster/abstreet/releases/download/v0.2.8/abstreet_mac_v0_2_8.zip
  - Unzip the directory, then run `play_abstreet.sh`.
  - If that just opens a text file instead of running the game, then instead
    open terminal, `cd` to the directory you just unzipped. Then do:
    `cd game; RUST_BACKTRACE=1 ./game 1> ../output.txt 2>&1`
  - [Help needed](https://github.com/dabreegster/abstreet/issues/77) to package
    this as a Mac .app, to make this process simpler
- Linux:
  https://github.com/dabreegster/abstreet/releases/download/v0.2.8/abstreet_linux_v0_2_8.zip
  - Unzip the directory, then run `play_abstreet.sh`.
- FreeBSD: https://www.freshports.org/games/abstreet/ (thanks to
  [Yuri](https://github.com/yurivict))

Or you can [compile from source](/docs/dev.md).

## Playing the game

- Use the **tutorial** to learn the controls.
- Play the **challenges** for directed gameplay.
- Try out any ideas in the **sandbox**.

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
- USGS SRTM

Other binary data bundled in:

- Overpass font (https://fonts.google.com/specimen/Overpass, Open Font License)
- Bungee fonts (https://fonts.google.com/specimen/Bungee, Open Font License)
- Material Design icons (https://material.io/resources/icons, Apache license)
