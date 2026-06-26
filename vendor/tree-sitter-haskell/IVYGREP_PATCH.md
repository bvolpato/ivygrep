# ivygrep patch

This directory vendors `tree-sitter-haskell` 0.23.1 from crates.io.

`src/tree_sitter/array.h` is replaced with commit
`6247f81e55392377ef96c58ae7eff037c995dfb9` from upstream pull request
[tree-sitter/tree-sitter-haskell#157](https://github.com/tree-sitter/tree-sitter-haskell/pull/157).
That patch updates the generated array helpers to avoid strict-aliasing undefined
behavior that corrupts the heap under optimized GCC builds on Linux ARM64.

Remove this patch only after a released `tree-sitter-haskell` version contains
the same fix and the `haskell_parser_safety` integration test passes against it.
