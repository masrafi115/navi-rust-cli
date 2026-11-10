# navi-rust-cli

An interactive Rust directory navigator inspired by `navi`, using Television's `frizbee` fuzzy-matching crate.

## Features

- Fuzzy-search directories in a chosen tree (defaults to your home directory).
- Navigate results with arrow keys and select with Enter; Esc or Ctrl-C cancels.
- Skips hidden directories, `node_modules`, and `target` while scanning.
- Prints the selected path to stdout, making shell integration straightforward.

## Install and run

```sh
cargo install --path .
navi-rust-cli                  # Search your home directory
navi-rust-cli --path ~/work     # Search a specific tree
navi-rust-cli --path ~/work -d 5
```

Add a shell helper so selection changes the current shell's directory:

```sh
# Bash or Zsh
function c { local dir; dir="$(navi-rust-cli \"$@\")" && [ -n "$dir" ] && cd -- "$dir"; }
```

Then run `c` or `c --path ~/work`.

## Fuzzy matching

Directory names are matched and ranked by [`frizbee`](https://github.com/saghen/frizbee), the SIMD fuzzy matcher used by [Television](https://github.com/alexpasmantier/television). Its query syntax supports fuzzy terms and operators such as `'term` (substring), `^term` (prefix), `term$` (suffix), and `!term` (negation).

## Development

```sh
cargo test
cargo run -- --help
```

Licensed under MIT. Fuzzy matching is provided by the MIT-licensed `frizbee` crate.
