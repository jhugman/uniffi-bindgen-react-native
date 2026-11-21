# @ubjs/electron

Runs a native uniffi library in an Electron utility process, and lets a sandboxed page call it through its `wasm2 --async` bindings.

Four subpaths, one for each process:

- `@ubjs/electron/main`: `serve()` forks a utility process for each window and hands the page its port.
- `@ubjs/electron/utility`: `serveUtility()` serves a library over `@ubjs/node`, in the utility process.
- `@ubjs/electron/preload`: a CommonJS file that gives the page a bridge to main.
- `@ubjs/electron/page`: `connect()` returns the port for the generated `uniffiInitAsync`, and `disconnected` says when the utility is gone.

See the [Electron reference](https://jhugman.github.io/uniffi-bindgen-react-native/reference/electron.html).

## Developing

Build the siblings first:

```sh
(cd ../../typescript && npm ci && npm run build)
(cd ../worker && npm ci)
(cd ../../runtimes/napi && npm ci --ignore-scripts && npm run build:debug)
```

`npm ci` in `channels/worker` builds it through its `prepare`.

Then install and build this package:

```sh
npm ci --ignore-scripts
npm run build
```

Without `--ignore-scripts`, npm runs the `prepare` of the linked `runtimes/napi`, which is a release `napi build`.

`npm test` runs the unit tests in plain Node, with no Electron.

`cargo test -- async_electron::`, from the repository root, runs every `--async` fixture under real Electron: its script in a sandboxed page, the library in a utility process. A lifecycle script adds a reload, a killed utility and a window close. Run `node node_modules/electron/install.js` here once first.
