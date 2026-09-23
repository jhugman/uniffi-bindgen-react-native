import { readFile } from "node:fs/promises";
import { run } from "./generated/generated-test.js";
const timeout = setTimeout(() => {
  throw new Error("generated JSPI fixture timed out");
}, 15000);
try {
  console.log(
    await run(
      await readFile(
        new URL("./generated/api/jspi_wasm2_codegen.wasm", import.meta.url),
      ),
    ),
  );
} finally {
  clearTimeout(timeout);
}
