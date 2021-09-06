DEFAULT: dev

SERVER_PORT=8000
REPO_ROOT:=$(shell git rev-parse --show-toplevel)

##
# Section: Tasks (Phony targets)
##

APPS=abstreet fifteen_min osm_viewer santa widgetry_demo map_editor

.PHONY: server clean dev release $(APPS)
.PRECIOUS: src/%/wasm_pkg

TSC=npx tsc --lib dom,es2020 --module es2020 --target es6 --strict --noEmitOnError --outDir build

clean:
	rm -fr build
	rm -fr src/*/wasm_pkg

# Build all apps for distribution (optimized)
# This assume's you'll upload any required data to the web-root separately.
release: export WASM_PACK_FLAGS=--release
# note: `clean` is because we don't know if any cached wasm files were optimized
# Once you've compiled the optimized wasm this way, you can skip `clean` and just run `make build`, since the remaining output is identical for dev vs release
release: clean build

# Build all apps for local dev, and symlink the data dir into the web-root.
dev: export WASM_PACK_FLAGS=--dev
dev: build build/dist/data

server: build/dist/data
	cd build/dist && python3 -m http.server $(SERVER_PORT)

##
# Section: Shared Deps and Templates
##

# Build all of our apps
build: $(APPS) build/dist/index.html

# Symlink in data for dev builds
build/dist/data:
	ln -sf ../../../data build/dist/data

build/dist/%.html: src/web_root/*.html
	# just copy everything... we could do something more nuanced
	cp -r src/web_root/* build/dist

# We rebuild wasm when any of these files change. 
#
# Because Make doesn't know about rust's build system, it's pretty naive:
# changing any rust file in any crate will trigger rebuilding the wasm 
# (the rebuilds themselves should be pretty fast due to incremental compilation)
RUST_SOURCES=$(shell find ../ -name \*.rs)

# Produce the wasm package via our wasm-pack script
src/%/wasm_pkg: $(RUST_SOURCES)
	bin/build-wasm $*
	touch $@

# simply copies over the wasm pkg we 
# built to be able to compile our typescript
build/dist/%/wasm_pkg: src/%/wasm_pkg
	mkdir -p $(dir $@)
	cp -r "${<}" $(dir $@)

# Concatenate the target crate's js file with the generic widgetry loading
# code.
#
# Alternatives and their downsides would be:
#  1. import outside of pkg root
#  	 - pkg is less portable
#  2. copy widgetry.js file into each package and update import path on build
#  	 - not really any less hacky, since we're still transforming at build
#  	 - an extra request for the client
#  3. use proper bundler like webpack and system.js:
#  	 - They're all complex - conceptually and dependency-wise
#  	 - We can't use AMD modules because it doesn't support `import.meta`
build/dist/%.bundle.js: build/widgetry.js build/%.js
	mkdir -p $(dir $@)
	bin/bundle-widgetry-js $^ > $@

build/%.js: src/%.ts
	$(TSC) $^

##
# Section: Apps
##

## A/BStreet

abstreet: build/dist/abstreet/wasm_pkg build/dist/abstreet/abstreet.bundle.js build/dist/abstreet build/dist/abstreet.html

# Unlike the other crates, we have an explicit rule to give the "game" js a
# more meaningful name
src/abstreet/wasm_pkg: $(RUST_SOURCES)
	bin/build-wasm game abstreet
	touch src/abstreet/wasm_pkg

build/dist/abstreet/abstreet.bundle.js: build/widgetry.js build/abstreet/abstreet.js

## Fifteen Minute Tool

fifteen_min: build/dist/fifteen_min/wasm_pkg build/dist/fifteen_min/fifteen_min.bundle.js build/dist/fifteen_min.html

build/dist/fifteen_min/fifteen_min.bundle.js: build/widgetry.js build/fifteen_min/fifteen_min.js

## OSM Viewer

osm_viewer: build/dist/osm_viewer/wasm_pkg build/dist/osm_viewer/osm_viewer.bundle.js build/dist/osm_viewer.html

build/dist/osm_viewer/osm_viewer.bundle.js: build/widgetry.js build/osm_viewer/osm_viewer.js

## Widgetry Demo

widgetry_demo: build/dist/widgetry_demo/wasm_pkg build/dist/widgetry_demo/widgetry_demo.bundle.js build/dist/widgetry_demo.html

build/dist/widgetry_demo/widgetry_demo.bundle.js: build/widgetry.js build/widgetry_demo/widgetry_demo.js

## Santa

santa: build/dist/santa/wasm_pkg build/dist/santa/santa.bundle.js build/dist/santa.html

build/dist/santa/santa.bundle.js: build/widgetry.js build/santa/santa.js

## RawMap editor

map_editor: build/dist/map_editor/wasm_pkg build/dist/map_editor/map_editor.bundle.js build/dist/map_editor.html

build/dist/map_editor/map_editor.bundle.js: build/widgetry.js build/map_editor/map_editor.js
