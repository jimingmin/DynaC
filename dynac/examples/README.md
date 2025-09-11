# DynaC examples

Run examples from the repo root using the crate manifest path:

```
cargo run --manifest-path dynac/Cargo.toml -- examples/00_hello.dc
```

Alternatively, from the `dynac/` directory:

```
cd dynac
cargo run -- ../examples/00_hello.dc
```

Files
- 00_hello.dc — hello world
- 01_variables.dc — variables and arithmetic
- 02_control_flow.dc — if/else, while, for
- 03_functions.dc — functions and closures
- 04_native_clock.dc — using native clock()
- 05_structs_stack_vs_heap.dc — struct literals, new, field access
- 06_structs_nested_and_promotion.dc — nested structs, promotion rules
- 07_traits_and_impls.dc — traits and impl methods using self
- 08_errors.dc — examples that intentionally error
