import * as wasm_pkg from "./wasm_pkg/fifteen_min.js";
let wasm_bg_path = "wasm_pkg/fifteen_min_bg.wasm";

import { InitInput, modRoot, WidgetryApp } from "../widgetry.js";

export class FifteenMinute extends WidgetryApp<wasm_pkg.InitOutput> {
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
