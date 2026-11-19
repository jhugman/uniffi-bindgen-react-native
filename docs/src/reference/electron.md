# Electron support

`@ubjs/electron` lets a sandboxed Electron page call a native UniFFI library. The library runs in a utility process, through `@ubjs/node`. The page uses the bindings that `ubrn generate wasm2 bindings --async` writes. A port connects the two.

There are four processes to wire up:

| Process | Subpath | What you write |
| ------- | ------- | -------------- |
| Main | `@ubjs/electron/main` | `serve(webContents, options)` |
| Utility | `@ubjs/electron/utility` | an entry that calls `serveUtility(definitions)` |
| Preload | `@ubjs/electron/preload` | nothing: a file that you load |
| Page | `@ubjs/electron/page` | `connect()`, then `uniffiInitAsync(port)` |

## Install

```sh
npm install @ubjs/electron @ubjs/node @ubjs/core @ubjs/worker
npm install --save-dev electron
```

Keep all four `@ubjs` packages at the same version.

List `@ubjs/node` yourself, even though `@ubjs/electron` depends on it. You bundle the utility entry and keep `@ubjs/node` external, so `require("@ubjs/node")` must resolve from your app. An installer that does not hoist, such as pnpm, does not make a transitive dependency resolvable there.

Keep `@ubjs/node` and `@ubjs/core` in `dependencies`, not `devDependencies`. Both load at runtime outside your bundles: `@ubjs/node` from the utility entry, and `@ubjs/core` from `@ubjs/node` when the library has async functions. electron-builder packages only `dependencies`. Without them a packaged utility fails at `register`, and the page falls back to wasm.

## Build the library

Build your crate as a `cdylib`:

```sh
cargo build --release
```

The result is `target/release/libmylib.dylib`, `.so` or `.dll`. The utility loads it by absolute path.

## Generate the page's bindings

Generate the `wasm2` bindings with `--async`, from the native library. No wasm32 build is needed.

```sh
ubrn generate wasm2 bindings --async --library --ts-dir src/generated target/release/libmylib.dylib
```

This writes `index.ts`, `index.port.ts` and `player-definitions.ts`, among others. The page imports `index.port.ts`. The utility imports `player-definitions.ts`.

```admonish warning
The page's bindings and the utility's library must come from the same build. The bindings carry the library's checksums. A mismatch does not fail `connect()`. It rejects `uniffiInitAsync` with a checksum mismatch error, and `uniffiInitAsync` keeps that rejection, so only a reload recovers. Regenerate the bindings whenever you rebuild the library.
```

## Wire the four processes

### Main

Call `serve()` for each window before `loadURL` or `loadFile`. Importing `@ubjs/electron/main` installs main's handler, so a page that connects to a window that `serve()` does not cover is told `not-served` at once.

```js
// main.cjs: a CommonJS main.
const path = require("node:path");
const { app, BrowserWindow } = require("electron");
const { serve } = require("@ubjs/electron/main");

app.whenReady().then(() => {
  const win = new BrowserWindow({
    webPreferences: { preload: require.resolve("@ubjs/electron/preload") },
  });
  const handle = serve(win.webContents, {
    entry: path.join(__dirname, "utility.cjs"),
    library: path.join(__dirname, "libmylib.dylib"),
    info: () => ({ dataDir: app.getPath("userData") }),
  });
  win.loadFile("index.html");
});
```

In an ES module main, use `createRequire(import.meta.url).resolve()` for `require.resolve()`, and `import.meta.dirname` for `__dirname`.

Here the library sits beside the main. In a packaged app that path is inside `app.asar`, and `serve()` maps it to `app.asar.unpacked`, where `asarUnpack` puts it.

`serve()` returns a handle. `handle.close()` stops serving the window and kills its utility. Closing the window does the same.

`ServeOptions`:

| Option | Meaning |
| ------ | ------- |
| `entry` | Absolute path of your bundled utility entry. |
| `library` | Absolute path of the cdylib. A path inside `app.asar` is rewritten to `app.asar.unpacked`, and the file must exist there. |
| `info` | Called with the `webContents`. What it returns must survive structured clone. The page and the utility both receive it, for example a data directory. |
| `inspect` | `true` starts each utility with `--inspect=0`, so each picks a free port. Ignored, with a warning, in a packaged app. |
| `allowFrame` | Which frames may connect. The default admits only the window's top frame. A filter that throws declines, with a warning. |
| `onUtility` | Receives `UtilityEvent`s. |

By default only the window's top frame may connect. There is one utility per `webContents`: if `allowFrame` admits several frames, each frame that connects replaces the one before, and the earlier frame sees `disconnected`. A second `webContents` gets its own utility, and a utility's death tells only its own page.

### Utility

The utility entry reads the library and the `info` that main sends, and serves the library.

```ts
import { serveUtility } from "@ubjs/electron/utility";
import { PLAYER_DEFINITIONS } from "./generated/player-definitions";

serveUtility(PLAYER_DEFINITIONS, {
  onInit: ({ library, info }) => {
    // Runs before the library opens. `info` is the value that main's `info()` returned.
  },
});
```

`UtilityInit` is `{ library, info }`. When the page's port closes, the utility exits with code 0. You do not call `unload()`.

Bundle this entry into one file, with `@ubjs/node` kept external. The `.cjs` name keeps it CommonJS in an app whose `package.json` says `"type": "module"`.

```sh
esbuild src/utility.ts --bundle --platform=node --format=cjs \
  --external:@ubjs/node --outfile=dist/utility.cjs
```

If `@ubjs/node` or the library fails to load, the utility exits 1, and the page's `connect()` rejects with `start-failed` and the cause.

### Preload

`@ubjs/electron/preload` is a CommonJS file that puts `window.__ubjsElectron` in the page. It needs `contextIsolation`, which is Electron's default. Load it in one of three ways:

- Set it as `webPreferences.preload`, if you have no preload of your own, as in the example above.
- Register it beside your own with a `Session`'s `registerPreloadScript({ type: "frame", filePath: require.resolve("@ubjs/electron/preload") })`, as in `session.defaultSession.registerPreloadScript(...)`. This reaches every window in that session, so give the windows you serve their own, for example `session.fromPartition("persist:app")`.
- `require` it for its side effect, from a preload that you bundle.

```admonish warning
A sandboxed preload can `require` only `electron`. An unbundled preload of your own cannot `require("@ubjs/electron/preload")`. Register the file beside yours instead.
```

The preload forwards only the replies for its page's own connection, and closes any other port. Main checks which frame asked.

### Page

```ts
import { connect, isAvailable, ConnectError } from "@ubjs/electron/page";
import { uniffiInitAsync, add } from "./generated/index.port";

if (isAvailable()) {
  try {
    const connection = await connect();
    await uniffiInitAsync(connection.port);
    console.log(await add(1, 2));

    connection.disconnected.then((info) => {
      console.warn("the library went away", info.code, info.reason);
    });
  } catch (e) {
    if (e instanceof ConnectError) {
      // Fall back to wasm.
    }
  }
}
```

```admonish note
Import `uniffiInitAsync` and the API from `./generated/index.port`, never from `./generated` or `./generated/index`. `generate wasm2 bindings --async` also writes `index.ts`, the wasm entry. It imports `@ubjs/wasm`, which a native-only app does not install.
```

`isAvailable()` says whether the preload ran. `connect({ timeoutMs })` resolves to a `Connection`. `timeoutMs` defaults to 30 000; `Infinity` waits forever. Outside a window, as under SSR, `isAvailable()` is `false` and `connect()` rejects with `unavailable`.

Load one copy of `@ubjs/electron/page` in a document. Two copies, as from a duplicated bundle, break each other: the first closes the second's port, and stops hearing when the utility dies.

- `port` is what you pass to `uniffiInitAsync`. `port.close()` ends the connection.
- `info` is what main's `info()` returned.
- `disconnected` is a promise of `DisconnectInfo`, which is `{ code, reason }`. `code` is the utility's exit code, or `null`.

`connect()` rejects with a `ConnectError`. Its `reason` is one of:

- `unavailable`: the preload did not run, or the page is not context isolated.
- `not-served`: main does not serve this window, or its `allowFrame` does not admit this frame. Call `serve()` before the window loads.
- `start-failed`: the utility did not start. The message names the cause.
- `timeout`: nothing answered within `timeoutMs`. Main never imported `@ubjs/electron/main`, or the utility took longer than that to start.

On any of them you can fall back to wasm. Every `connect()` in a document gets the first call's promise, so a failed `connect()` stays failed until the page reloads.

## When the utility dies

When the utility crashes or is killed, `disconnected` settles and every call in flight rejects with a `ChannelClosedError`. Match it by `name`, not with `instanceof`, because the page and the generated code may bundle separate copies:

```ts
if (e instanceof Error && e.name === "ChannelClosedError") { /* ... */ }
```

There is no reconnect. A reload is the only recovery. Main starts a new utility when the reloaded page calls `connect()`.

Two other things end a page's connection the same way: `handle.close()` and closing the window.

If the page calls `connection.port.close()`, `disconnected` settles as `{ code: null, reason: "closed" }`.

## Debug

A utility's `stdout` and `stderr` reach main's terminal, so Rust's logging shows there.

`inspect` prints each utility's `ws://` URL on its stderr.

A utility that fails to start sends main its stack, and main's console shows it.

`onUtility` reports what happens to each utility:

- `spawn`, with the pid.
- `ready`, once the library is open and the page has its port.
- `exit`, with the code and `expected`.

An exit is `expected` when main retired the utility for a new request or a teardown, or when it exited 0 after its port closed, as on every reload. A failed start or a crash is not expected. To report crashes, log on `!expected`.

## Package the app

- Unpack the native files. Add the `.node` file and your library to `asarUnpack`. `serve()` rewrites an `app.asar` path to `app.asar.unpacked` and checks that the file is there.
- Sign the library with the rest of the app.
- `@ubjs/node` loads a prebuilt addon from a package named for the platform, which installs for the machine you build on: `@ubjs/node-<platform>-<arch>` on macOS, and the same with `-gnu`, `-musl` or `-msvc` after it on Linux and Windows, as in `@ubjs/node-linux-x64-gnu`. To build for the other architecture, install that package too.
- For a universal macOS build, install both `@ubjs/node-darwin-arm64` and `@ubjs/node-darwin-x64`, and unpack both `.node` files. No universal `@ubjs/node` package is published. Then either ship a universal library made with `lipo -create`, or choose the library's path by `process.arch` in `serve({ library })`.

## Security

Serve only windows that load your own content. The utility runs native code with your app's rights, and any frame that may connect can call it, whatever it has navigated to. If a window can reach content you do not control, pass `allowFrame` to admit only your own frames, and block that navigation, as Electron's security checklist advises.

The page is trusted with the native library's memory safety, not only its API. Object handles cross the port as raw pointers into the utility's memory, so script that runs in an admitted frame, an XSS in your own content included, can forge one and crash the utility or corrupt its memory.

Load the preload only where you need it. `registerPreloadScript` puts the bridge in every window of the session, remote ones too. Main answers an unserved window with `not-served`, but a narrower bridge is easier to reason about.

## Limitations

- There is no reconnect. After `disconnected`, the only recovery is a reload.
- `serve()` serves one library per `webContents`.
- Two windows on one data directory run two utilities on one store. Give each window its own directory in `info`.
- A utility stuck forever in a synchronous call after its page left lingers until the window closes. A new request retires it first, so two never overlap.
- A synchronous `RustBuffer` return is copied three times in the utility: by `@ubjs/node` under Electron's memory cage, by the receiver, and by structured clone. Peak memory is about three times the buffer.
- A preview of `@ubjs/electron` needs a released `@ubjs/node` that includes async entries.
- CI runs the Electron tests on Linux under xvfb. On macOS they run locally.
