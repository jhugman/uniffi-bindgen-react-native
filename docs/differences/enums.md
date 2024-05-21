Rust has enums with an associated struct of properties:

```rs
enum FilePath {
  Local { path: String }
  Remote { host: String, path: String }
}
```

They are constructed:

```rs
let my_file = FilePath::Local { path: "/some/place".to_string() };
```

They pattern match quite nicely:

```rs
fn get_full_path(file_path: &FilePath) -> String {
    match file_path {
        FilePath::Local { path } => path.clone(),
        FilePath::Remote { path, host } => format!("{host}/{path}"),
    }
}
```

In typescript, we don't have enums with properties:

```ts
enum FilePathKind {
    LOCAL = "Local",
    REMOTE = "Remote",
};

type FilePath =
    { kind: FilePathKind.LOCAL, value: { path: string } } |
    { kind: FilePathKind.REMOTE, value: { host: string, path: string } };
```

I'm not sure how well this constructs in practice, but the pattern matching seems to work well:

```ts
function getPath(filePath: FilePath): string {
    switch (filePath.kind) {
        case FilePathKind.LOCAL:
            return filePath.value.path;
        case FilePathKind.REMOTE: {
            const { host, path } = filePath.value;
            return `${host}/${path}`;
        }
    }
}
```

### Enums with tuples

Rust also has enums that can accept tuples:

```rs
enum NumberUnit {
    Meters(f64),
    Kg(f64),
    Seconds(f64),
}
```

I'm fairly sure uniffi supports these, but I haven't seen them in the current project:
