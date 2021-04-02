// WidgetryApp is a wrapper for a rust Widgetry app which has been compiled to a wasm package using wasm_bindgen.
//
// The type signatures of `InitInput` and `initializeWasm` were copy/pasted from the wasm_bindgen
// generated ts.d files. They should be stable, unless wasm_bindgen has breaking changes.
export type InitInput =
  | RequestInfo
  | URL
  | Response
  | BufferSource
  | WebAssembly.Module;

export abstract class WidgetryApp<InitOutput> {
  private appLoader: AppLoader<InitOutput>;
  private _assetsBaseURL: string;
  private _assetsAreGzipped: boolean;

  public constructor(domId: string) {
    this.appLoader = new AppLoader(this, domId);

    // Assume a default relative path to where we can find the "system" dir
    // Overide with `myApp.setAssetsBaseURL('path/to/dir')`
    this._assetsBaseURL = "./data";

    // Assume files are gzipped unless on localhost.
    // Overide with `myApp.setAssetsAreGzipped(true)`
    this._assetsAreGzipped = !isLocalhost;
  }

  public async loadAndStart() {
    this.appLoader.loadAndStart();
  }

  // Assets (the "system" dir) are assumed to be at "./data" relative
  // to the current URL. Otherwise override with `setAssetsBaseURL`.
  public assetsBaseURL(): string {
    return this._assetsBaseURL;
  }

  public setAssetsBaseURL(newValue: string) {
    this._assetsBaseURL = newValue;
  }

  // Assets are assumed to gzipped, unless on localhost
  public assetsAreGzipped(): boolean {
    return this._assetsAreGzipped;
  }

  public setAssetsAreGzipped(newValue: boolean) {
    this._assetsAreGzipped = newValue;
  }

  abstract initializeWasm(
    module_or_path?: InitInput | Promise<InitInput>
  ): Promise<InitOutput>;
  abstract run(
    rootDomId: string,
    assetsBaseURL: string,
    assetsAreGzipped: boolean
  ): void;
  abstract wasmURL(): string;
}

enum LoadState {
  unloaded,
  loading,
  loaded,
  starting,
  started,
  error,
}

/**
 * Helper class used by `WidgetryApp` implementations to load their wasm and
 * render their content.
 */
export class AppLoader<T> {
  app: WidgetryApp<T>;
  el: HTMLElement;
  loadingEl?: HTMLElement;
  errorEl?: HTMLElement;
  domId: string;
  state: LoadState = LoadState.unloaded;
  // (receivedLength, totalLength)
  downloadProgress?: [number, number];
  errorMessage?: string;

  public constructor(app: WidgetryApp<T>, domId: string) {
    this.app = app;
    this.domId = domId;
    const el = document.getElementById(domId);
    if (el === null) {
      throw new Error(`element with domId: ${domId} not found`);
    }
    this.el = el;
    console.log("sim constructor", this);
  }

  public async loadAndStart() {
    this.render();
    try {
      await this.load();
      await this.start();
    } catch (e) {
      this.reportErrorState(e.toString());
      throw e;
    }
  }

  async load() {
    console.assert(this.state == LoadState.unloaded, "already loaded");
    this.updateState(LoadState.loading);

    console.log("Started loading WASM");
    const t0 = performance.now();
    let response: Response = await fetch(this.app.wasmURL());

    if (response.body == null) {
      this.reportErrorState("response.body was unexpectedly null");
      return;
    }
    let reader = response.body.getReader();

    let contentLength = response.headers.get("Content-Length");
    if (contentLength == undefined) {
      this.reportErrorState(
        "unable to fetch wasm - contentLength was unexpectedly undefined"
      );
      return;
    }

    if (response.status == 404) {
      this.reportErrorState(
        `server misconfiguration, wasm file not found: ${this.app.wasmURL()}`
      );
      return;
    }

    this.downloadProgress = [0, parseInt(contentLength)];

    let chunks: Uint8Array[] = [];
    while (true) {
      const { done, value } = await reader.read();
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
    let buffer = await blob.arrayBuffer();
    const t1 = performance.now();
    console.log(`It took ${t1 - t0} ms to download WASM, now initializing it`);

    // TODO: Prefer streaming instantiation where available (not safari)? Seems like it'd be faster.
    // const { instance } = await WebAssembly.instantiateStreaming(response, imports);

    //let imports = {};
    //let instance = await WebAssembly.instantiate(bytes, imports);

    await this.app.initializeWasm(buffer);
    this.updateState(LoadState.loaded);
  }

  async start() {
    console.assert(this.state == LoadState.loaded, "not yet loaded");
    this.updateState(LoadState.starting);
    try {
      console.log(
        `running app with assetsBaseURL: ${this.app.assetsBaseURL()}, assetsAreGzipped: ${this.app.assetsAreGzipped()}`
      );
      this.app.run(
        this.domId,
        this.app.assetsBaseURL(),
        this.app.assetsAreGzipped()
      );
    } catch (e) {
      if (
        e.toString() ==
        "Error: Using exceptions for control flow, don't mind me. This isn't actually an error!"
      ) {
        // This is an expected, albeit unfortunate, control flow mechanism for winit on wasm.
        this.updateState(LoadState.started);
      } else {
        throw e;
      }
    }
  }

  isWebGL1Supported(): boolean {
    try {
      var canvas = document.createElement("canvas");
      return !!canvas.getContext("webgl");
    } catch (e) {
      return false;
    }
  }

  isWebGL2Supported(): boolean {
    try {
      var canvas = document.createElement("canvas");
      return !!canvas.getContext("webgl2");
    } catch (e) {
      return false;
    }
  }

  updateState(newValue: LoadState) {
    console.debug(
      `state change: ${LoadState[this.state]} -> ${LoadState[newValue]}`
    );
    this.state = newValue;
    this.render();
  }

  reportErrorState(errorMessage: string) {
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
          let progressText = `${prettyPrintBytes(
            received
          )} / ${prettyPrintBytes(total)}`;
          let percentText = `${(100.0 * received) / total}%`;
          this.loadingEl.querySelector<HTMLElement>(
            ".widgetry-app-loader-progress-text"
          )!.innerText = progressText;
          this.loadingEl.querySelector<HTMLElement>(
            ".widgetry-app-loader-progress-bar"
          )!.style.width = percentText;
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

export function modRoot(importMetaURL: string): string {
  function dirname(path: string): string {
    return path.match(/.*\//)!.toString();
  }

  let url = new URL(importMetaURL);
  url.pathname = dirname(url.pathname).toString();
  return url.toString();
}

function buildLoadingEl(): HTMLElement {
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

function buildErrorEl(errorMessage: string): HTMLElement {
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

function prettyPrintBytes(bytes: number): string {
  if (bytes < 1024 ** 2) {
    return Math.round(bytes / 1024) + " KB";
  }
  return Math.round(bytes / 1024 ** 2) + " MB";
}

// courtesy: https://stackoverflow.com/a/57949518
const isLocalhost = Boolean(
  window.location.hostname === "localhost" ||
    window.location.hostname === "0.0.0.0" ||
    // [::1] is the IPv6 localhost address.
    window.location.hostname === "[::1]" ||
    // 127.0.0.1/8 is considered localhost for IPv4.
    window.location.hostname.match(
      /^127(?:\.(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)){3}$/
    )
);
