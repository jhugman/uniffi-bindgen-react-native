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

## Uniffi Reserved words

In your Rust code, avoid using identifiers beginning with the word `uniffi` or `Uniffi`.
