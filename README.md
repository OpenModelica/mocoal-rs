# mocoal-rs

Gives the components of a Modelica model a `Placement` and its connections a `Line`. For models
written as text, without graphical annotations.

The Rust implementation of
[ModelicaComponentAutolayout.jl](https://github.com/JKRT/ModelicaComponentAutolayout.jl): the
same command line, the same C interface, the same output. Two forms, one crate:

- the standalone program `mocoal`,
- the shared library `libmocoal` (C interface, for tools such as OMEdit).

The README of ModelicaComponentAutolayout.jl describes the options, the rules of the layout and
the limitations. This file has what differs.

## Build

Rust 1.99 (stable) and git.

```bash
scripts/fetch-omc.sh
```

```bash
cargo build --release
```

`target/release/mocoal` and `target/release/libmocoal.{so,dylib}` (`mocoal.dll` on Windows).
macOS arm64: `mocoal` 1.6 MB, `libmocoal.dylib` 1.5 MB, no other files needed at run time.

## Parser

The source is parsed with OpenModelica's own Rust parser, the crate `openmodelica_ast`
(`OMCompiler/Compiler/OpenModelica.rs` in
[OpenModelica](https://github.com/OpenModelica/OpenModelica)). It gives the `Absyn` tree of omc.
The crate is linked statically: no OpenModelica installation is needed to build or run.

`scripts/fetch-omc.sh` copies 5 crates (`openmodelica_ast`, `openmodelica_error`,
`openmodelica_wasi`, `metamodelica`, `metamodelica_derive`) and 6 builtin `.mo` files into
`omc/` (1.5 MB), at the commit in [omc/COMMIT](omc/COMMIT). They are not in this repository.
`OPENMODELICA_SRC=<checkout> scripts/fetch-omc.sh` takes them from a local checkout, else they
come from GitHub (38 s). A new parser: change `omc/COMMIT`, fetch, build, run the checks.

## Interface

[interface/INTERFACE.md](interface/INTERFACE.md) is the specification of the library and
[interface/test_lib.c](interface/test_lib.c) the C program that tests it. Both are copies: the
files are kept in ModelicaComponentAutolayout.jl (`INTERFACE.md`, `juliac/test_lib.c`). A change
of the interface goes into both repositories; `scripts/check.sh sync` fails when a copy differs.

```c
char *mocoal_layout(const char *text, const char *file_name, const char *options);
const char *mocoal_message(void);
const char *mocoal_version(void);
void mocoal_free(char *text);
```

The library also exports 3 symbols of the parser crates (`omrs_add_runtime_error`,
`omrs_add_runtime_error_pos`, `omrs_runtime_abort`). They are not part of the interface.

## Source

One module per file of the Julia package (`src/extract.rs` mirrors `src/extract.jl`), 7900
lines. Names follow the rules of the Rust omc: `snake_case` functions, `CamelCase` types,
`SCREAMING_SNAKE_CASE` constants, rustfmt with `max_width = 120`, errors as `Result`.

`src/numbers.rs` has `sind`, `cosd` and the printing of numbers as Julia does them: the output
has to be the same to the last digit.

## Checks

```bash
cargo test
```

```bash
scripts/check.sh all
```

```bash
cargo fmt -p mocoal -- --check
```

```bash
cargo clippy -p mocoal --all-targets --no-deps -- -D warnings
```

55 tests. `check.sh [exe|lib|sync|all]` runs the program on
[test/models/Chain.mo](test/models/Chain.mo), compiles and runs `interface/test_lib.c` against
the library, and compares the copies in `interface/` with a checkout of
ModelicaComponentAutolayout.jl (`MOCOAL_JL`, default `../ModelicaComponentAutolayout.jl`).
`cargo fmt` and `cargo clippy` need `-p mocoal` (and clippy `--no-deps`): without it they work on
the fetched crates too.

`scripts/compare.sh MOCOAL_A MOCOAL_B LIBRARY_DIR WORK_DIR [options]` lays out a copy of a
library with two programs and compares the files, the SVG previews and the reports.

The workflow [.github/workflows/build.yml](.github/workflows/build.yml) runs the checks (without
`sync`), builds both forms and uploads them as artifacts: Linux x86_64 on a push or pull
request; Linux, macOS arm64 and Windows x86_64 on a run by hand (Actions > Build > Run
workflow).

## Compared with the Julia program

`mocoal` 0.1.0 of both, macOS arm64, `-i --svg`, compared with `scripts/compare.sh`. Files,
SVG previews and reports are the same, byte for byte:

| Library | Files | Changed | SVG | Option sets |
|---|---|---|---|---|
| Modelica 4.1.0 | 2552 | 975 | 1182 | 8 |
| Buildings 13.0.0 | 5621 | 2622 | 2639 | 1 |
| Modelica 3.2.3 | 212 | 137 | 1014 | 1 |
| SystemDynamics 2.1.2 | 1 | 1 | 69 | 1 |

`mocoal -n Modelica` (4.1.0): 1.4 s for both. Size: 1.6 MB against 43 MB (the Julia program
with its runtime).

Two differences:

- The text of a parse error is the parser's. `model A Real x end A;` gives
  `a.mo:1:16: No viable alternative near token: end` here, and
  `Expected ';' after the element but found 'end A'` at the same position in Julia.
- A file that is not UTF-8 is refused (`FILE: not UTF-8`; `the text is not UTF-8` in the
  library). The Julia program passes the bytes on.

## Licence

AGPL v3 or OSMC-PL 1.8, according to the recipient's choice. See [LICENSE.md](LICENSE.md).
