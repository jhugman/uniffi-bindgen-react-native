import { readFile } from "node:fs/promises";
import { run } from "./generated/test.js";

const timeout = setTimeout(() => {
  throw new Error("JSPI fixture timed out");
}, 15000);
try {
  console.log(
    await run(
      await readFile(
        new URL("./generated/ts/wasm-bindgen/index_bg.wasm", import.meta.url),
      ),
    ),
  );
} finally {
  clearTimeout(timeout);
}
