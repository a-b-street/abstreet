# Development notes

Find packages to upgrade: `cargo outdated -R`

Diff screencaps: http://www.imagemagick.org/Usage/compare/#methods

Debug OpenGL calls:

```
apitrace trace --api gl ../target/debug/editor ../data/raw_maps/montlake.abst
qapitrace editor.trace
apitrace dump editor.trace
```

## Profiling

`apt-get install google-perftools libgoogle-perftools-dev`

Follow Usage from https://crates.io/crates/cpuprofiler

Run editor or headless with `--enable_profiler`

```
google-pprof --no_strip_temp ../target/debug/editor profile
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
cross build --release --target x86_64-pc-windows-gnu --bin editor
wine target/x86_64-pc-windows-gnu/release/editor.exe data/maps/montlake.abst
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

https://github.com/ekalinin/github-markdown-toc for table of contents

## Videos

```
# Fullscreen
ffmpeg -f x11grab -r 25 -s 1920x1080 -i :0.0 -vcodec huffyuv raw.avi
# Default window
ffmpeg -f x11grab -r 25 -s 1024x768 -i :0.0+28,92 -vcodec huffyuv raw.avi

ffmpeg -ss 10.0 -t 5.0 -i raw.avi -f gif -filter_complex "[0:v] fps=12,scale=1024:-1,split [a][b];[a] palettegen [p];[b][p] paletteuse" screencast.gif
```
