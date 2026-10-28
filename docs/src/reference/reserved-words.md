# Reserved words

## Typescript Reserved Words

The following words are reserved words in Typescript.

If the Rust API uses any of these words on their own, the generated typescript is appended with an underscore (`_`).

Reserved Words | Strict Mode Reserved Words
---------------| ---------------------------
`break`        | `as`
`case`         | `implements`
`catch`        | `interface`
`class`        | `let`
`const`        | `package`
`continue`     | `private`
`debugger`     | `protected`
`default`      | `public`
`delete`       | `static`
`do`           | `yield`
`else`
`enum`
`export`
`extends`
`false`
`finally`
`for`
`function`
`if`
`import`
`in`
`instanceof`
`new`
`null`
`return`
`super`
`switch`
`this`
`throw`
`true`
`try`
`typeof`
`var`
`void`
`while`
`with`

e.g.

```rust
#[uniffi::export]
fn void() {}
```

generates valid Typescript:

```typescript
function void_() {
  // … call into Rust.
}
```

## `Error` is mapped to `Exception`

Due to the relative prevalence in idiomatic Rust of an error enum called `Error`, and the built-in `Error` class in Javascript, an `Error` enum is renamed to `Exception`.

## Other Javascript globals get an underscore

A type named like any other standard Javascript global (`Object`, `Map`, `Set`, `Promise`, `Symbol`, `Date`, `Uint8Array`, …) would shadow that global in the generated module, which relies on several of them. These types are generated with an underscore appended, as reserved words are: a Rust `Object` is `Object_` in Typescript.

## Uniffi Reserved words

In your Rust code, avoid using identifiers beginning with the word `uniffi` or `Uniffi`.
