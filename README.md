# DynaC

DynaC is a small dynamic language with a bytecode VM, garbage collection, closures, first-class functions, structs, and traits/impl-style methods.


## Build the `dynac` binary first

Prerequisites
- Rust toolchain (stable). Install via rustup.

Build (release recommended)
- From the repo root, build the executable named `dynac`:

```
cargo build --release --manifest-path dynac/Cargo.toml
```

The binary will be at:
- `dynac/target/release/dynac` (Linux/macOS)

Optionally install to your PATH:

```
cargo install --path dynac
```

This makes the `dynac` command available globally.

## Run your scripts

Using the built binary directly:

```
./dynac/target/release/dynac path/to/your_script.dc
```

If installed with `cargo install`:

```
dynac path/to/your_script.dc
```

Run the REPL (no file argument):

```
dynac
```

Examples
- Run the provided examples with the `dynac` binary:

```
dynac examples/00_hello.dc
```


## Grammar (informal reference)

Literals and identifiers
- Numbers: decimal integers and floats (e.g., 42, 3.14)
- Strings: double-quoted (e.g., "hello")
- Booleans: true, false
- Nil: nil
- Identifiers: ASCII letters, digits, underscores; must not start with a digit
- Line comments: // … to end of line

Program structure
- A program is a sequence of declarations and statements, terminated by semicolons where required.
- Blocks are delimited by braces: { declarationOrStatement* }

Declarations
- Variable declaration:
	- var name = expression? ;
	- Uninitialized vars default to nil.
- Function declaration:
	- fn name ( parameters? ) { block }
	- Parameters are comma-separated; closures and nested functions are supported.
- Struct declaration:
	- struct TypeName { field ( , field )* ,? }
	- Field list may include a trailing comma.
- Trait declaration:
	- trait TraitName { ( fn methodName ( parameters? ) ; )* }
	- Only method signatures; each ends with a semicolon.
- Impl declaration:
	- impl TraitName for TypeName { ( fn methodName ( parameters? ) { block } )* }
	- Methods receive an implicit self receiver; refer to fields via self.field.

Statements
- Expression statement: expression ;
- Print statement: print expression ;
- Return statement: return expression? ; (only inside functions)
- If statement:
	- if ( expression ) statement ( else statement )?
- While statement:
	- while ( expression ) statement
- For statement (C-style):
	- for ( initializer? ; condition? ; increment? ) statement
	- initializer may be a var declaration or an expression statement or empty; condition and increment are optional.
- Block:
	- { declarationOrStatement* }

Expressions
- Precedence (low → high):
	1. or
	2. and
	3. equality: ==, !=
	4. comparison: <, <=, >, >=
	5. term: +, -
	6. factor: *, /
	7. unary: !, -
	8. call, property access: ( … ), .name
- Grouping: ( expression )
- Function call: callee ( arguments? ) with comma-separated arguments
- Property/field access: receiver.name
- Assignment forms:
	- Variable: name = expression
	- Field: receiver.name = expression

Structs and instances
- Stack-allocated literal:
	- TypeName { field = expression ( , field = expression )* ,? }
	- Intended for local (frame) use; may not be returned from a function.
- Heap-allocated instance:
	- new TypeName { field = expression ( , field = expression )* ,? }
	- Use this to return or store globally; behaves like a reference type.
- Field access and assignment via .field

Traits and method calls
- Define a trait with method signatures:
	- trait Printable { fn print_self(); }
- Implement for a type:
	- impl Printable for string { fn print_self() { print "hi"; } }
- Call methods on instances: instance.method(args)

Built-ins
- print expression; writes a textual representation to stdout.
- clock() → number (milliseconds since UNIX epoch).

Semicolons and whitespace
- Semicolons are required after declarations and statements (e.g., var, return, print, expression statements).
- Whitespace and newlines are generally insignificant outside tokens.

Notes
- Returning a stack-allocated struct literal is a compile error; use new TypeName { … } when returning or escaping a value.
- Method bodies can capture outer variables (closures); trait impl methods also support upvalue capture.
