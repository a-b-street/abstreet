import * as wasm_pkg from "./wasm_pkg/ltn.js";
let wasm_bg_path = "wasm_pkg/ltn_bg.wasm";

import { InitInput, modRoot, WidgetryApp } from "../widgetry.js";

export class LTN extends WidgetryApp<wasm_pkg.InitOutput> {
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
