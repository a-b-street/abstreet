# Web Stuff

This is a collection of API's and build tools for packaging our various Widgetry
apps as web applications.

## Goals

A web developer, who might not know anything about rust or wasm, should be able
to use our packaged javascript libraries on their website with minimal
customization or arcanery.

Users of their website should be able to interact with the widgetry app without
it feeling weird or having to jump through hoops.

## Limitations

### JS feature: `import.meta`

To allow the application to live at any URL (rather than presupposing it lives
at root, or whatever), we rely on `import.meta` which isn't supported on some
browsers before 2018. See: https://caniuse.com/?search=import.meta

An alternative would be to require configuration, so the loader knows where to
download it's "\*\_wasm.bg file".

### Browser Feature: WebGL

We prefer WebGL2, but now gracefully fall back to WebGL1. This should cover all
common browsers since late 2014. https://caniuse.com/?search=webgl

## Examples

See [`src/web_root/*.js`](examples/) for code examples.

You can build and see the examples in your webbrowser with:

```
// install typescript build dependency
npm install
make dev
make server
```

The workflow for interactive development of just one app in debug mode:

```
./bin/build-wasm game abstreet && rm -rf build/dist/abstreet/wasm_pkg/ && cp -Rv src/abstreet/wasm_pkg build/dist/abstreet/ && make server
```
