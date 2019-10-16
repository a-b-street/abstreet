# Development notes

Find packages to upgrade: `cargo outdated -R`

Deal with compile tile: `cargo bloat --time`

Find why two binary crates aren't sharing dependencies:
https://old.reddit.com/r/rust/comments/cqceu4/common_crates_in_cargo_workspace_recompiled/

Where's a dependency coming from? `cargo tree -i -p syn`

Diff screencaps: http://www.imagemagick.org/Usage/compare/#methods

Debug OpenGL calls:

```
apitrace trace --api gl ../target/debug/game ../data/raw_maps/montlake.bin
qapitrace game.trace
apitrace dump game.trace
```

Understand XML: just use firefox

## Profiling

`apt-get install google-perftools libgoogle-perftools-dev`

Follow Usage from https://crates.io/crates/cpuprofiler

Modify `editor/Cargo.toml` to include the `profiler` feature on `ezgui`. Then
run game or headless with `--enable_profiler`

```
google-pprof --no_strip_temp ../target/debug/game profile
google-pprof --no_strip_temp ../target/release/headless profile
top30 --cum
```

## Building releases

Cross-compilation notes: https://github.com/rust-embedded/cross Or use
https://github.com/japaric/trust

Initially have to:

```shell
rustup target add x86_64-pc-windows-gnu
```

Then:

```
sudo systemctl start docker
cross build --release --target x86_64-pc-windows-gnu --bin game
wine target/x86_64-pc-windows-gnu/release/game.exe data/maps/montlake.bin
```

## Markdown

For formatting:

```
sudo apt-get install npm
cd ~; mkdir npm; cd npm
npm init --yes
npm install prettier --save-dev --save-exact
```

Use https://github.com/joeyespo/grip to render. Doesn't seem to work with the
graphviz image.

https://github.com/ekalinin/github-markdown-toc for table of contents (stashed
in ~/Downloads/gh-md-toc)

## Videos

```
# Fullscreen
ffmpeg -f x11grab -r 25 -s 1920x1080 -i :0.0 -vcodec huffyuv raw.avi
# Default window
ffmpeg -f x11grab -r 25 -s 1800x800 -i :0.0+28,92 -vcodec huffyuv raw.avi

ffmpeg -ss 10.0 -t 5.0 -i raw.avi -f gif -filter_complex "[0:v] fps=12,scale=1024:-1,split [a][b];[a] palettegen [p];[b][p] paletteuse" screencast.gif
```

## JOSM

```
java -jar ~/Downloads/josm-tested.jar ~/abstreet/map_editor/parking_diff.osc
```

Press (and release T), then click to pan. Download a relevant layer, select the
.osc, merge, then upload.

## Fresh data

http://download.geofabrik.de/north-america/us/washington.html

wget http://download.geofabrik.de/north-america/us/washington-latest.osm.pbf

osmosis --read-pbf ~/Downloads/washington-latest.osm.pbf --bounding-polygon file=~/abstreet/data/polygons/huge_seattle.poly completeWays=true --write-xml ~/Seattle.osm
