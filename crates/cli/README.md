[![crates.io](https://img.shields.io/crates/v/glory-cli)](https://crates.io/crates/glory-cli)
[![Discord](https://img.shields.io/discord/1031524867910148188?color=%237289DA&label=discord)](https://discord.gg/YdRAhS7eQB)

Build tool for [Glory](https://crates.io/crates/glory):

[<img src="https://raw.githubusercontent.com/gbj/glory/main/docs/logos/Glory_logo_RGB.png" alt="Glory Logo" style="width: 30%; height: auto; display: block; margin: auto;">](http://https://crates.io/crates/glory)

<br/>

- [Features](#features)
- [Getting started](#getting-started)
  - [Dependencies](#dependencies)
- [Single-package setup](#single-package-setup)
- [Workspace setup](#workspace-setup)
- [Build features](#build-features)
- [Parameters reference](#parameters-reference)
  - [Compilation parameters](#compilation-parameters)
  - [Site parameters](#site-parameters)
  - [Environment variables](#environment-variables)
  - [End-to-end testing](#end-to-end-testing)

<br/>

# Features

- Parallel build of server and client in watch mode for fast developer feedback.
- CSS hot-reload (no page-reload, only CSS updated).
- Build server and client for hydration (client-side rendering mode not supported).
- Support for both workspace and single-package setup.
- SCSS compilation using [dart-sass](https://sass-lang.com/dart-sass).
- CSS transformation and minification using [Lightning CSS](https://lightningcss.dev).
- Builds server and client (wasm) binaries using Cargo.
- Generates JS - Wasm bindings with [wasm-bindgen](https://crates.io/crates/wasm-bindgen)
  - Includes support for [JS Snippets](https://rustwasm.github.io/docs/wasm-bindgen/reference/js-snippets.html#js-snippets) for when you want to call some JS code from your WASM.
- Optimises the wasm with _wasm-opt_ from [Binaryen](https://github.com/WebAssembly/binaryen)
- `watch` command for automatic rebuilds with browser live-reload.
- `test` command for running tests of the lib and bin packages that makes up the Glory project.
- `build` build the server and client.
- `end2end` command for building, running the server and calling a bash shell hook. The hook would typically launch Playwright or similar.
- `new` command for creating a new project based on templates, using [cargo-generate](https://cargo-generate.github.io/cargo-generate/index.html). Current templates include
  - [`https://github.com/glory-rs/start`](https://github.com/glory-rs/start): An Actix starter
  - [`https://github.com/glory-rs/start-salvo`](https://github.com/glory-rs/start-salvo): An salvo starter
  - [`https://github.com/glory-rs/start-salvo-workspace`](https://github.com/glory-rs/start-salvo-workspace): An salvo starter keeping client and server code in separate crates in a workspace
- 'no_downloads' feature to allow user management of optional dependencies
  <br/>

# Getting started

Install:

> `cargo install --locked glory-cli`

If you, for any reason, need the bleeding-edge super fresh version:

> `cargo install --git https://github.com/glory-rs/glory-cli --locked glory-cli`

Help:

> `cargo glory --help`

For setting up your project, have a look at the [examples](https://github.com/glory-rs/glory-cli/tree/main/examples)

<br/>

## Dependencies

The dependencies for [sass](https://sass-lang.com/install), [wasm-opt](https://github.com/WebAssembly/binaryen) and
[cargo-generate](https://github.com/cargo-generate/cargo-generate#installation) are automatically installed in a cache directory
when they are used if they are not already installed and found by [which](https://crates.io/crates/which).
Different versions of the dependencies might accumulate in this directory, so feel free to delete it.

| OS      | Example                                   |
| ------- | ----------------------------------------- |
| Linux   | /home/alice/.cache/glory-cli           |
| macOS   | /Users/Alice/Library/Caches/glory-cli  |
| Windows | C:\Users\Alice\AppData\Local\glory-cli |

If you wish to make it mandatory to install your dependencies, or are using Nix or NixOs, you can
install it with the `no_downloads` feature enabled to prevent glory-cli from trying to download and install them.

> `cargo install --features no_downloads --locked glory-cli`

<br/>

# Single-package setup

The single-package setup is where the code for both the frontend and the server is defined in a single package.

Configuration parameters are defined in the package `Cargo.toml` section `[package.metadata.glory]`. See the Parameters reference for
a full list of parameters that can be used. All paths are relative to the package root (i.e. to the `Cargo.toml` file)

<br/>

# Workspace setup

When using a workspace setup both single-package and multi-package projects are supported. The latter is when the frontend
and the server reside in different packages.

All workspace members whose `Cargo.toml` define the `[package.metadata.glory]` section are automatically included as Glory
single-package projects. The multi-package projects are defined on the workspace level in the `Cargo.toml`'s
section `[[workspace.metadata.glory]]` which takes three mandatory parameters:

```toml
[[workspace.metadata.glory]]
# project name
name = "glory-project"
bin-package = "server"
lib-package = "front"

# more configuration parameters...
```

Note the double braces: several projects can be defined and one package can be used in several projects.

<br/>

# Build features

When building with glory-cli, the frontend, library package, is compiled into wasm using target
`wasm-unknown-unknown` and the features `--no-default-features --features=web-ssr --features=web-csr`
The server binary is compiled with the features `--no-default-features --features=web-ssr`

<br/>

# Parameters reference

These parameters are used either in the workspace section `[[workspace.metadata.glory]]` or the package,
for single-package setups, section `[package.metadata.glory]`.

Note that the Cargo Manifest uses the word _target_ with two different meanings.
As a package's configured `[[bin]]` targets and as the compiled output target triple.
Here, the latter is referred to as _target-triple_.

## Compilation parameters

```toml
# Sets the name of the binary target used.
#
# Optional, only necessary if the bin_package defines more than one target
bin_target = "my_bin_name"

# The features to use when compiling all targets
#
# Optional. Can be extended with the command line parameter __features
features = []

# The features to use when compiling the bin target
#
# Optional. Can be over_ridden with the command line parameter __bin_features
bin_features = ["ssr"]

# If the __no_default_features flag should be used when compiling the bin target
#
# Optional. Defaults to false.
bin_default_features = false

# The profile to use for the bin target when compiling for release
#
# Optional. Defaults to "release".
bin_profile_release = "my_release_profile"

# The profile to use for the bin target when compiling for debug
#
# Optional. Defaults to "debug".
bin_profile_debug = "my_debug_profile"

# The target triple to use when compiling the bin target
#
# Optional. Env: GLORY_BIN_TARGET_TRIPLE
bin_target_triple = "x86_64_unknown_linux_gnu"

# The features to use when compiling the lib target
#
# Optional. Can be over_ridden with the command line parameter __lib_features
lib_features = ["hydrate"]

# If the __no_default_features flag should be used when compiling the lib target
#
# Optional. Defaults to false.
lib_default_features = false

# The profile to use for the lib target when compiling for release
#
# Optional. Defaults to "release".
lib_profile_release = "my_release_profile"

# The profile to use for the lib target when compiling for debug
#
# Optional. Defaults to "debug".
lib_profile_debug = "my_debug_profile"
```

## Site parameters

These parameters can be overridden by setting the corresponding environment variable. They can also be
set in a `.env` file as glory_cli reads the first it finds in the package or workspace directory and
any parent directory.

```toml
# Sets the name of the output js, wasm and css files.
#
# Optional, defaults to the lib package name or, in a workspace, the project name. Env: GLORY_OUTPUT_NAME.
output_name = "myproj"

# The site root folder is where glory_cli generate all output.
# NOTE: It is relative to the workspace root when running in a workspace.
# WARNING: all content of this folder will be erased on a rebuild.
#
# Optional, defaults to "target/site". Env: GLORY_SITE_ROOT.
site_root = "target/site"

# The site_root relative folder where all compiled output (JS, WASM and CSS) is written.
#
# Optional, defaults to "pkg". Env: GLORY_SITE_PKG_DIR.
site_pkg_dir = "pkg"

# The source style file. If it ends with _.sass_ or _.scss_ then it will be compiled by `dart_sass`
# into CSS and processed by lightning css. When release is set, then it will also be minified.
#
# Optional. Env: GLORY_STYLE_FILE.
style_file = "style/main.scss"

# The tailwind input file.
#
# Optional, Activates the tailwind build
tailwind_input_file = "style/tailwind.css"

# The tailwind config file.
#
# Optional, defaults to "tailwind.config.js" which if is not present
# is generated for you
tailwind_config_file = "tailwind.config.js"

# The browserlist https://browsersl.ist query used for optimizing the CSS.
#
# Optional, defaults to "defaults". Env: GLORY_BROWSERQUERY.
browser_query = "defaults"

# Assets source dir. All files found here will be copied and synchronized to site_root.
# The assets_dir cannot have a sub directory with the same name/path as site_pkg_dir.
#
# Optional. Env: GLORY_ASSETS_DIR.
assets_dir = "assets"

# JS source dir. `wasm_bindgen` has the option to include JS snippets from JS files
# with `#[wasm_bindgen(module = "/js/foo.js")]`. A change in any JS file in this dir
# will trigger a rebuild.
#
# Optional. Defaults to "src"
js_dir = "src"

# The IP and port where the server serves the content. Use it in your server setup.
#
# Optional, defaults to 127.0.0.1:8000. Env: GLORY_SITE_ADDR.
site_addr = "127.0.0.1:8000"

# The port number used by the reload server (only used in watch mode).
#
# Optional, defaults 3001. Env: GLORY_RELOAD_PORT
reload_port = 3001

# The command used for running end_to_end tests. See the section about End_to_end testing.
#
# Optional. Env: GLORY_END2END_CMD.
end2end_cmd = "npx playwright test"

# The directory from which the end_to_end tests are run.
#
# Optional. Env: GLORY_END2END_DIR
end2end_dir = "integration"
```

<br/>

## Environment variables

The following environment variables are set when compiling the lib (front) or bin (server) and when the server is run.

Echoed from the Glory config:

- GLORY_OUTPUT_NAME
- GLORY_SITE_ROOT
- GLORY_SITE_PKG_DIR
- GLORY_SITE_ADDR
- GLORY_RELOAD_PORT

Directories used when building:

- GLORY_LIB_DIR: The path (relative to the working directory) to the library package
- GLORY_BIN_DIR: The path (relative to the working directory) to the binary package

Note when using directories:

- `glory-cli` changes the working directory to the project root or if in a workspace, the workspace root before building and running.
- the two are set to the same value when running in a single-package config.
- Avoid using them at run-time unless you can guarantee that the entire project struct is available at runtime as well.

## End-to-end testing

`glory-cli` provides end-to-end testing support for convenience. It is a simple
wrapper around a shell command `end2end-cmd` that is executed in a specific directory `end2end-dir`.

The `end2end-cmd` can be any shell command. For running [Playwright](https://playwright.dev) it
would be `npx playwright test`.

What it does is equivalent to running this manually:

- in a terminal, run `cargo glory watch`
- in a separate terminal, change to the `end2end-dir` and run the `end2end-cmd`.

When testing the setup, please try the above first. If that works but `cargo glory end-to-end`
doesn't then please create a GitHub ticket.
