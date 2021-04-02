import * as wasm_pkg from "./wasm_pkg/osm_viewer.js";
let wasm_bg_path = "wasm_pkg/osm_viewer_bg.wasm";

import { InitInput, modRoot, WidgetryApp } from "../widgetry.js";

export class OSMViewer extends WidgetryApp<wasm_pkg.InitOutput> {
  initializeWasm(
    module_or_path?: InitInput | Promise<InitInput>
  ): Promise<wasm_pkg.InitOutput> {
    return wasm_pkg.default(module_or_path);
  }

  run(rootDomId: string, assetsBaseURL: string, assetsAreGzipped: boolean) {
    wasm_pkg.run(rootDomId, assetsBaseURL, assetsAreGzipped);
  }

  wasmURL(): string {
    return modRoot(import.meta.url) + wasm_bg_path;
  }
}
