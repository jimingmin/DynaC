# DynaC examples

Run examples from the repo root using the crate manifest path:

```
cargo run --manifest-path dynac/Cargo.toml -- examples/00_hello.lat
```

Alternatively, from the `dynac/` directory:

```
cd dynac
cargo run -- ../examples/00_hello.lat
```

Files
- 00_hello.lat — hello world
- 01_variables.lat — variables and arithmetic
- 02_control_flow.lat — if/else, while, for
- 03_functions.lat — functions and closures
- 04_native_clock.lat — using native clock()
- 05_structs_stack_vs_heap.lat — struct literals, new, field access
- 06_structs_nested_and_promotion.lat — nested structs, promotion rules
- 07_traits_and_impls.lat — traits and impl methods using self
- 08_errors.lat — examples that intentionally error
