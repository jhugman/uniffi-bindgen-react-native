# Contributing or reviewing documentation

A project is only as good as its docs!

The documentation is in [markdown](https://rust-lang.github.io/mdBook/format/markdown.html), and lives in the `docs/src` directory.

You can edit the files directly with a text editor.

## Before you start

The following assumes you have checked out the `uniffi-bindgen-react-native` project and that Rust is installed.

## Install `mdbook`

The docs are produced by [`mdbook`, a static-site generator](https://rust-lang.github.io/mdBook/index.html) written for documenting Rust projects.

`uniffi-bindgen-react-native` uses this with a few plugins. You can install it by opening the terminal and using `cd` to navigate to the project directory, then running the following command:

```sh
./scripts/run-bootstrap-docs.sh
```

## Run `mdbook serve`

`mdbook` can now be run from the `docs` directory.

From within the project directory, run the following:

```sh
cd docs
mdbook serve
```

This will produce output like:

```sh
2024-10-14 12:59:35 [INFO] (mdbook::book): Book building has started
2024-10-14 12:59:35 [INFO] (mdbook::book): Running the html backend
2024-10-14 12:59:35 [INFO] (mdbook::book): Running the linkcheck backend
2024-10-14 12:59:35 [INFO] (mdbook::renderer): Invoking the "linkcheck" renderer
2024-10-14 12:59:36 [INFO] (mdbook::cmd::serve): Serving on: http://localhost:3000
2024-10-14 12:59:36 [INFO] (warp::server): Server::run; addr=[::1]:3000
2024-10-14 12:59:36 [INFO] (warp::server): listening on http://[::1]:3000
```

## Make some changes

You can edit pages with your text editor.

New pages should be added to the `SUMMARY.md` file so that a) `mdbook` knows about them and b) they ends up in the table of contents.

You can now navigate your browser to [localhost:3000](http://localhost:3000/) to see the changes you've made.

## Pushing these changes back into the project

A normal Pull Request flow is used to push these changes back into the project.

---
