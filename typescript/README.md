# @ubjs/core

Runtime support for [`uniffi-bindgen-javascript`][bindgen] generated
bindings. Imported by generated TypeScript code; not intended for
direct use.

[bindgen]: https://github.com/jhugman/uniffi-bindgen-react-native

## Compatibility

Ships dual ESM/CJS builds via an `exports` field; works in Node ≥18,
Bun, React Native (Metro, RN ≥0.71), and modern web bundlers.

## Relationship to `uniffi-bindgen-react-native`

`@ubjs/core` and `uniffi-bindgen-react-native` ship the same runtime
bytes under different package names. The legacy name remains on npm
so older generated code keeps resolving its imports; new generated
code imports `@ubjs/core`. They are version-locked.

## Install

You should not install this directly. Your generated bindings list
`@ubjs/core` as a peer requirement when you regenerate them with a
recent `ubrn` CLI.

```bash
npm install @ubjs/core
```
