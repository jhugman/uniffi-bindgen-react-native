# test-runner

Hermes and JSI demo of JSI Host Functions loaded dynamically from
shared libraries specified on the command line.

`test-runner` dynamically loads shared libraries specified on the command line
after the input JS files, and calls them to register any native functions into
the global object. Then it executes the input JS file, which can use the registered
natives.

It is a lightly edited version taken from the CC0 licenced [`hf-runner`](https://github.com/tmikov/hermes-jsi-demos/tree/master/hf-runner).

## Usage

```
test-runner <js-file> [<shared-lib> ...]
```

## Shared Library API

The shared libraries must export a function named `registerNatives` with the following
signature:

```c
void registerNatives(facebook::jsi::Runtime &rt);
```
