# Third-party licences

This package is the MIT-licensed engine of [formel](https://github.com/manuel-rhdt/formel) compiled
to WebAssembly. The binary therefore contains code from the libraries below; each keeps its own
licence, and all of them are permissive.

| Crate | Licence (SPDX) | Project |
| :--- | :--- | :--- |
| [formel](https://crates.io/crates/formel) | MIT | https://github.com/manuel-rhdt/formel |
| [math-core](https://crates.io/crates/math-core) | MIT | https://github.com/tmke8/math-core |
| [harfrust](https://crates.io/crates/harfrust) | MIT | https://github.com/harfbuzz/harfrust |
| [read-fonts](https://crates.io/crates/read-fonts) | MIT OR Apache-2.0 | https://github.com/googlefonts/fontations |
| [quick-xml](https://crates.io/crates/quick-xml) | MIT | https://github.com/tafia/quick-xml |
| [svg](https://crates.io/crates/svg) | MIT OR Apache-2.0 | https://github.com/bodoni/svg |
| [wasm-bindgen](https://crates.io/crates/wasm-bindgen) | MIT OR Apache-2.0 | https://github.com/wasm-bindgen/wasm-bindgen |

These in turn depend on the crates pinned in the repository's `Cargo.lock` — `font-types`,
`unicode-properties`, `phf`, `bitflags` and so on — under MIT, Apache-2.0 or (for `memchr`) the
Unlicense. `cargo license` or `cargo deny list` prints the exact set with each crate's licence text.

The package ships **no fonts**. The OpenType MATH font you load keeps its own licence; see the
repositories in the [demo's licence list](https://manuel-rhdt.github.io/formel/fonts/LICENSES.html)
for the free ones and their terms.
