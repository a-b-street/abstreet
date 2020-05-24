# Development notes

Find packages to upgrade: `cargo outdated -R`

Deal with compile tile: `cargo bloat --time`

Find why two binary crates aren't sharing dependencies:
https://old.reddit.com/r/rust/comments/cqceu4/common_crates_in_cargo_workspace_recompiled/

Where's a dependency coming from? `cargo tree -i -p syn`

Diff screencaps: http://www.imagemagick.org/Usage/compare/#methods

Debug OpenGL calls:

```
apitrace trace --api gl ../target/debug/game ../data/input/raw_maps/montlake.bin
qapitrace game.trace
apitrace dump game.trace
```

Understand XML: just use firefox

## Profiling

Actually, https://github.com/flamegraph-rs/flamegraph is pretty cool too.

`apt-get install google-perftools libgoogle-perftools-dev`

Follow Usage from https://crates.io/crates/cpuprofiler

Modify `game/Cargo.toml` to include the `profiler` feature on `ezgui`. Then
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
cargo install cross
sudo apt-get install docker.io
sudo usermod -aG docker ${USER}
```

Then:

```
sudo systemctl start docker
cross build --release --target x86_64-pc-windows-gnu --bin game
wine target/x86_64-pc-windows-gnu/release/game.exe data/system/maps/montlake.bin
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
ffmpeg -f x11grab -r 25 -s 1920x960 -i :0.0+0,55 -vcodec huffyuv raw.avi

ffmpeg -ss 10.0 -t 5.0 -i raw.avi -f gif -filter_complex "[0:v] fps=12,scale=1024:-1,split [a][b];[a] palettegen [p];[b][p] paletteuse" screencast.gif
```

## JOSM

```
java -jar ~/Downloads/josm-tested.jar ~/abstreet/map_editor/diff.osc
```

Press (and release T), then click to pan. Download a relevant layer, select the
.osc, merge, then upload.

## Faster linking

```
sudo apt-get install lld
```

Stick this in ~/.cargo/config:

```
[target.x86_64-unknown-linux-gnu]                                                                   
rustflags = [                                                                                       
    "-C", "link-arg=-fuse-ld=lld",                                                    
]
```

## git

Keep a fork up to date:

```
# Once
git remote add upstream https://github.com/rust-windowing/glutin/

git fetch upstream
git merge upstream/master
git diff upstream/master
```

## Refactoring

perl -pi -e 's/WrappedComposite::text_button\(ctx, (.+?), (.+?)\)/Btn::text_fg(\1).build_def\(ctx, \2\)/' `find|grep rs|xargs`

## Stack overflow

rust-gdb --args ../target/release/game --dev

## Drawing diagrams

draw.io

## Mapping

xodo on Android for annotating maps in the field
