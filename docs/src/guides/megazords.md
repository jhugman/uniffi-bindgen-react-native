# Working with multiple crates in one library

Some teams arrange their Rust library in to multiple crates, or multiple teams from one organization combine their efforts into one library.

This might be for better code organization, or to reduce shipping multiple copies of the same dependencies.

The combined library from multiple crates, in Mozilla vernacular, is known as a [Megazord](https://robots.fandom.com/wiki/Mighty_Morphin%27_Megazord).

`uniffi-rs` and `uniffi-bindgen-react-native` both work well with Megazords.

`uniffi-bindgen-react-native` produces a cluster of files per crate. For example, generating files from the library `libmymegazord.a` might contain two crates, `crate1` and `crate2`. The library directory would look like this:

```
cpp
├── generated
│   ├── crate1.cpp
│   ├── crate1.hpp
│   ├── crate2.cpp
│   └── crate2.hpp
├── react-native-my-megazord.cpp
└── react-native-my-megazord.h
src
├── NativeMyMegazord.ts
├── generated
│   ├── crate1.ts
│   ├── crate1-ffi.ts
│   ├── crate2.ts
│   └── crate2-ffi.ts
└── index.tsx
```

In `index.tsx`, the types are re-exported from `crate1.ts` and `crate2.ts`.

In this extended example, `crate1.ts` might declare a `Crate1Type` and `crate2.ts` a `Crate2Type`.

In this case, your library's client code would import `Crate1Type` and `Crate2Type` like this:

```ts
import { Crate1Type, Crate2Type } from "react-native-my-megazord";
```

Alternatively, they can use the default export:

```ts
import megazord from "react-native-my-megazord";

const { Crate1Type } = megazord.crate1;
const { Crate2Type } = megazord.crate2;
```

```admonish warning title="Duplicated identifiers"
Due to Swift's large granular module sytem, crates in the same megazord cannot have types of the same name.

This may be solved in Swift at some point— e.g. by adding prefixes— but until then, duplicate identifiers will cause a Typescript compilation error as the types are smooshed together in `index.tsx`.
```
