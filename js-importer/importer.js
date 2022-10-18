import init, { one_step_import } from "./pkg/js_importer.js";

await init();

export async function oneStepImport(x) {
	return one_step_import(x);
}
