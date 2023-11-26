var __awaiter = (this && this.__awaiter) || function (thisArg, _arguments, P, generator) {
    function adopt(value) { return value instanceof P ? value : new P(function (resolve) { resolve(value); }); }
    return new (P || (P = Promise))(function (resolve, reject) {
        function fulfilled(value) { try { step(generator.next(value)); } catch (e) { reject(e); } }
        function rejected(value) { try { step(generator["throw"](value)); } catch (e) { reject(e); } }
        function step(result) { result.done ? resolve(result.value) : adopt(result.value).then(fulfilled, rejected); }
        step((generator = generator.apply(thisArg, _arguments || [])).next());
    });
};
export class WidgetryApp {
    constructor(domId) {
        this.appLoader = new AppLoader(this, domId);
        // Assume a default relative path to where we can find the "system" dir
        // Override with `myApp.setAssetsBaseURL('path/to/dir')`
        this._assetsBaseURL = "./data";
        // Assume files are gzipped unless on localhost.
        // Override with `myApp.setAssetsAreGzipped(true)`
        this._assetsAreGzipped = !isLocalhost;
    }
    loadAndStart() {
        return __awaiter(this, void 0, void 0, function* () {
            this.appLoader.loadAndStart();
        });
    }
    // Assets (the "system" dir) are assumed to be at "./data" relative
    // to the current URL. Otherwise override with `setAssetsBaseURL`.
    assetsBaseURL() {
        return this._assetsBaseURL;
    }
    setAssetsBaseURL(newValue) {
        this._assetsBaseURL = newValue;
    }
    // Assets are assumed to gzipped, unless on localhost
    assetsAreGzipped() {
        return this._assetsAreGzipped;
    }
    setAssetsAreGzipped(newValue) {
        this._assetsAreGzipped = newValue;
    }
}
var LoadState;
(function (LoadState) {
    LoadState[LoadState["unloaded"] = 0] = "unloaded";
    LoadState[LoadState["loading"] = 1] = "loading";
    LoadState[LoadState["loaded"] = 2] = "loaded";
    LoadState[LoadState["starting"] = 3] = "starting";
    LoadState[LoadState["started"] = 4] = "started";
    LoadState[LoadState["error"] = 5] = "error";
})(LoadState || (LoadState = {}));
/**
 * Helper class used by `WidgetryApp` implementations to load their wasm and
 * render their content.
 */
export class AppLoader {
    constructor(app, domId) {
        this.state = LoadState.unloaded;
        this.app = app;
        this.domId = domId;
        const el = document.getElementById(domId);
        if (el === null) {
            throw new Error(`element with domId: ${domId} not found`);
        }
        this.el = el;
        console.log("sim constructor", this);
    }
    loadAndStart() {
        return __awaiter(this, void 0, void 0, function* () {
            this.render();
            try {
                yield this.load();
                yield this.start();
            }
            catch (e) {
                this.reportErrorState(e.toString());
                throw e;
            }
        });
    }
    load() {
        return __awaiter(this, void 0, void 0, function* () {
            console.assert(this.state == LoadState.unloaded, "already loaded");
            this.updateState(LoadState.loading);
            console.log("Started loading WASM");
            const t0 = performance.now();
            let response = yield fetch(this.app.wasmURL());
            if (response.body == null) {
                this.reportErrorState("response.body was unexpectedly null");
                return;
            }
            let reader = response.body.getReader();
            let contentLength = response.headers.get("Content-Length");
            if (contentLength == undefined) {
                this.reportErrorState("unable to fetch wasm - contentLength was unexpectedly undefined");
                return;
            }
            if (response.status == 404) {
                this.reportErrorState(`server misconfiguration, wasm file not found: ${this.app.wasmURL()}`);
                return;
            }
            this.downloadProgress = [0, parseInt(contentLength)];
            let chunks = [];
            while (true) {
                const { done, value } = yield reader.read();
                if (done) {
                    break;
                }
                if (value == undefined) {
                    console.error("reader value was unexpectedly undefined");
                    break;
                }
                chunks.push(value);
                this.downloadProgress[0] += value.length;
                this.render();
            }
            let blob = new Blob(chunks);
            let buffer = yield blob.arrayBuffer();
            const t1 = performance.now();
            console.log(`It took ${t1 - t0} ms to download WASM, now initializing it`);
            // TODO: Prefer streaming instantiation where available (not safari)? Seems like it'd be faster.
            // const { instance } = await WebAssembly.instantiateStreaming(response, imports);
            //let imports = {};
            //let instance = await WebAssembly.instantiate(bytes, imports);
            yield this.app.initializeWasm(buffer);
            this.updateState(LoadState.loaded);
        });
    }
    start() {
        return __awaiter(this, void 0, void 0, function* () {
            console.assert(this.state == LoadState.loaded, "not yet loaded");
            this.updateState(LoadState.starting);
            try {
                console.log(`running app with assetsBaseURL: ${this.app.assetsBaseURL()}, assetsAreGzipped: ${this.app.assetsAreGzipped()}`);
                this.app.run(this.domId, this.app.assetsBaseURL(), this.app.assetsAreGzipped());
            }
            catch (e) {
                if (e.toString() ==
                    "Error: Using exceptions for control flow, don't mind me. This isn't actually an error!") {
                    // This is an expected, albeit unfortunate, control flow mechanism for winit on wasm.
                    this.updateState(LoadState.started);
                }
                else {
                    throw e;
                }
            }
        });
    }
    isWebGL1Supported() {
        try {
            var canvas = document.createElement("canvas");
            return !!canvas.getContext("webgl");
        }
        catch (e) {
            return false;
        }
    }
    isWebGL2Supported() {
        try {
            var canvas = document.createElement("canvas");
            return !!canvas.getContext("webgl2");
        }
        catch (e) {
            return false;
        }
    }
    updateState(newValue) {
        console.debug(`state change: ${LoadState[this.state]} -> ${LoadState[newValue]}`);
        this.state = newValue;
        this.render();
    }
    reportErrorState(errorMessage) {
        this.errorMessage = errorMessage;
        this.updateState(LoadState.error);
    }
    // UI
    render() {
        this.el.style.backgroundColor = "black";
        switch (this.state) {
            case LoadState.loading: {
                if (this.loadingEl == undefined) {
                    this.loadingEl = buildLoadingEl();
                    // insert after rendering initial progress to avoid jitter.
                    this.el.append(this.loadingEl);
                }
                if (this.downloadProgress != undefined) {
                    let received = this.downloadProgress[0];
                    let total = this.downloadProgress[1];
                    let progressText = `${prettyPrintBytes(received)} / ${prettyPrintBytes(total)}`;
                    let percentText = `${(100.0 * received) / total}%`;
                    this.loadingEl.querySelector(".widgetry-app-loader-progress-text").innerText = progressText;
                    this.loadingEl.querySelector(".widgetry-app-loader-progress-bar").style.width = percentText;
                }
                break;
            }
            case LoadState.error: {
                if (this.loadingEl != undefined) {
                    this.loadingEl.remove();
                    this.loadingEl = undefined;
                }
                if (this.errorEl == undefined) {
                    if (!this.isWebGL1Supported() && !this.isWebGL2Supported()) {
                        this.errorMessage =
                            this.errorMessage +
                                "😭 Looks like your browser doesn't support WebGL.";
                    }
                    if (this.errorMessage == undefined) {
                        this.errorMessage =
                            "An unknown error occurred. Try checking the developer console.";
                    }
                    let el = buildErrorEl(this.errorMessage);
                    this.errorEl = el;
                    this.el.append(el);
                }
                break;
            }
        }
    }
}
export function modRoot(importMetaURL) {
    function dirname(path) {
        return path.match(/.*\//).toString();
    }
    let url = new URL(importMetaURL);
    url.pathname = dirname(url.pathname).toString();
    return url.toString();
}
function buildLoadingEl() {
    let loadingEl = document.createElement("div");
    loadingEl.innerHTML = `
        <style type="text/css">
            .widgetry-app-loader {
                color: white;
                padding: 16px;
            }
            .widgetry-app-loader-progress-bar-container {
                background-color: black;
                border: 1px solid white;
                border-radius: 4px;
            }
            .widgetry-app-loader-progress-bar {
                background-color: white; 
                height: 12px;
            }
            .widgetry-app-loader-progress-text {
                margin-bottom: 16px;
            }
        </style>
        <p><strong>Loading...</strong></p>
        <div class="widgetry-app-loader-progress-bar-container" style="width: 100%;">
            <div class="widgetry-app-loader-progress-bar" style="width: 1%;"></div>
        </div>
        <div class="widgetry-app-loader-progress-text">0 / 0</div>
        <p>If you think something has broken, check your browser's developer console (Ctrl+Shift+I or similar)</p>
        <p>(Your browser must support WebGL and WebAssembly)</p>
    `;
    loadingEl.setAttribute("class", "widgetry-app-loader");
    return loadingEl;
}
function buildErrorEl(errorMessage) {
    let el = document.createElement("p");
    el.innerHTML = `
      <style type="text/css">
        .widgetry-app-loader-error {
          color: white;
          text-align: center;
          padding: 16px;
        }
      </style>
      <h2>Error Loading App</h2>
      ${errorMessage}
    `;
    el.setAttribute("class", "widgetry-app-loader-error");
    return el;
}
function prettyPrintBytes(bytes) {
    if (bytes < Math.pow(1024, 2)) {
        return Math.round(bytes / 1024) + " KB";
    }
    return Math.round(bytes / Math.pow(1024, 2)) + " MB";
}
// courtesy: https://stackoverflow.com/a/57949518
const isLocalhost = Boolean(window.location.hostname === "localhost" ||
    window.location.hostname === "0.0.0.0" ||
    // [::1] is the IPv6 localhost address.
    window.location.hostname === "[::1]" ||
    // 127.0.0.1/8 is considered localhost for IPv4.
    window.location.hostname.match(/^127(?:\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)){3}$/));
import * as wasm_pkg from "./wasm_pkg/ltn.js";
let wasm_bg_path = "wasm_pkg/ltn_bg.wasm";
export class LTN extends WidgetryApp {
    initializeWasm(module_or_path) {
        return wasm_pkg.default(module_or_path);
    }
    run(rootDomId, assetsBaseURL, assetsAreGzipped) {
        wasm_pkg.run(rootDomId, assetsBaseURL, assetsAreGzipped);
    }
    wasmURL() {
        return modRoot(import.meta.url) + wasm_bg_path;
    }
}
