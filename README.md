# offstage

![build](https://github.com/tjni/offstage/workflows/build/badge.svg)

## Usage

An example best illustrates how to use offstage.

```sh
offstage prettier --write
```

Running this in a Git repository which has `src/A.js` and `src/B.js` in the
staging area will execute

```sh
offstage prettier --write src/A.js src/B.js
```

If modifications occur to `src/A.js` or `src/B.js`, they will be automatically
added to the staging area.

## Options

```sh
offstage --help

offstage 0.1.0

USAGE:
    offstage [OPTIONS] --shell <shell> [command]...

FLAGS:
    -h, --help       Prints help information
    -V, --version    Prints version information

OPTIONS:
    -f, --filter <filter>    Glob pattern to filter staged files
    -s, --shell <shell>      Shell executable to use to run the command [env: SHELL=/usr/bin/fish]

ARGS:
    <command>...    Command to run on staged files
```

## Developing

[Install Rust](https://www.rust-lang.org/learn/get-started).

Run the CLI during development:

```sh
cargo run <arguments>
```

Run tests:

```sh
cargo test
```

Create a release build:

```sh
cargo build --release
ls -alh target/release/offstage
```

## Attribution

This would not exist if not for the inspiration and methodology from the amazing
<b>[lint-staged](https://github.com/okonet/lint-staged)</b> project.
