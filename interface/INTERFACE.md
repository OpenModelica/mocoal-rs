# The interface of libmocoal

Interface version 1.

This is what a program that loads the library (OMEdit) may rely on, and what an implementation
of the library has to do. Nothing below depends on the implementation, except the section
[This implementation](#this-implementation).

## Functions

```c
char *mocoal_layout(const char *text, const char *file_name, const char *options);
const char *mocoal_message(void);
const char *mocoal_version(void);
void mocoal_free(char *text);
```

Four names with C linkage and the C calling convention. There is no header file: these
declarations are the header. Every text is UTF-8 and ends with a NUL.

### mocoal_layout

Gives the components of the classes in a Modelica source a `Placement` and the connections a
`Line`.

| Argument | Content | `NULL` or `""` |
|---|---|---|
| `text` | The content of one Modelica file: an optional `within`, then classes. | A text without classes. |
| `file_name` | The path of the file the text comes from. Used in messages and in the [class lookup](#class-lookup). The file is not read and not written. | No file. |
| `options` | The [options](#options). | The defaults. |

Result:

- The new source, in memory of the library. Release it with `mocoal_free`, not with `free`.
- When there is nothing to change, a copy of `text`.
- `NULL` when there is no layout: a parse error in `text`, a word in `options` that is not
  known, or an internal error. `mocoal_message` gives the reason.

### mocoal_message

After a `NULL`: the error. After a result: the warnings, one per line, or `""`.

The text belongs to the library and is valid until the next `mocoal_layout`. It is English text
for people. Its wording is not part of the interface: a caller tests the result of
`mocoal_layout` for `NULL` and shows the message.

### mocoal_version

The version of the implementation (`"0.1.1"`). The text belongs to the library and is valid as
long as the library is loaded. It is not the version of the interface (see
[Open points](#open-points)).

### mocoal_free

Releases a result of `mocoal_layout`. `NULL` is allowed.

## What a result is

1. Only annotations differ between `text` and the result: the `Placement` of components, the
   annotation of `connect` equations, and the `Diagram` of the class annotation (`graphics`, and
   the `extent` of its `coordinateSystem`). Everything else is the same byte for byte: comments,
   white space, line ends (LF or CRLF), a byte order mark.
2. With `--class`, text outside the named classes is the same byte for byte. OMEdit relies on
   this: it passes the text of a file and takes the class back out of the result
   (`AutoLayout::layoutClass`).
3. The result parses. The implementation parses its result before it returns it, and returns
   `NULL` otherwise.
4. A layout of a result, with the same options, returns that result.
5. The same `text`, `file_name`, `options` and files on disk give the same result. There are no
   random numbers and no clock in the layout.
6. Nothing is written: no file, no standard output, no standard error. Files are read only for
   the class lookup.
7. An error inside the library gives `NULL`, not an exception or a jump across the interface.

## Options

`options` is a command line: words separated by white space.

- A word in double quotes keeps its spaces: `-L "/my libs"`. There is no escape character, so a
  word cannot contain a double quote.
- An option with a value is `--name value` or `--name=value`; the short names take
  `-c value`.
- Numbers have a decimal point (`1.5`), independent of the locale, and are finite.

| Option | Value | Default | Meaning |
|---|---|---|---|
| `-c`, `--class NAME` | a class name | all classes | Lay out only this class and the classes in it. May be repeated. `NAME` is the full name (`P.A`) or its end (`A` matches `P.A` and `Q.A`). |
| `-k`, `--keep` | | off | Keep existing placements and lines, add what is missing. |
| `--layout NAME` | `layered`, `spring`, `grid` | `layered` | The placement. |
| `--route NAME` | `orthogonal`, `straight`, `bezier`, `none` | `orthogonal` | The lines. `none` writes no lines. |
| `--direction NAME` | `right`, `down` | `right` | The direction of the signal flow. |
| `--spacing DX[,DY]` | numbers ≥ 0 | `30,20` | The space between components. |
| `--scale S` | a number > 0 | `1` | Factor on the component size. |
| `--no-rotate` | | off | Do not turn components whose connectors are all acausal. |
| `--graphics NAME` | `move`, `keep`, `drop` | `move` | What happens to the shapes of the Diagram layer. |
| `-L`, `--library DIR` | a directory | none | A directory with libraries, for the class lookup. May be repeated; searched in the order given. |
| `--stack MB` | an integer from 1 to 65536 | `256` | The stack of the layout in MB. A level of nesting in the model takes about 3 KB. |

What the options do is described in [README.md](README.md).

Rules:

- Any other word gives `NULL` and a message: an unknown option, a missing value, an unknown
  value, a file name, and the options of the program `mocoal` that are not layout options
  (`-o`, `--stdin`, `-i`, `-n`, `-q`, `--svg`, `-h`, `--version`).
- No layout is made with a part of the options. An implementation that does not have an option
  or a value returns `NULL`; it does not ignore the word.
- A text without classes comes back as it is. So `mocoal_layout("", NULL, "--graphics move")`
  tells whether the library knows `--graphics move`: `""` when it does, `NULL` when not.
- A `--class` that matches no class is not an error: the result is `text`, and the message is a
  warning.

## Class lookup

The layout needs the class of every component: the size of its icon and the places of its
connectors. The library finds classes itself, in this order:

1. the classes in `text`;
2. when the file `file_name` exists: the directory that holds its top-level package (the
   directory of the file, one level up for a `package.mo` and one per name in the `within`),
   so the other files of the package and the libraries beside it;
3. the `--library` directories, in the order given;
4. the directories in the environment variables `OPENMODELICALIBRARY`, then `MODELICAPATH`
   (separated by `:`, on Windows by `;`);
5. `$HOME/.openmodelica/libraries`.

In a directory, the library `Name` is `Name/package.mo`, `Name.mo`, `Name VERSION/package.mo` or
`Name VERSION.mo`. With several versions: the one a `uses` annotation names, else the highest.

A class that is not found is not an error. Its component gets the default size, the places of
its connectors are unknown, and the message has a warning
(`A: class Nope.B of b not found; default size, ports unknown`).

Consequences for a caller that has classes in memory:

- Only `text` and files are seen. A class that is loaded in the caller but not on disk is not
  found.
- A class in another file is read as it is on disk, without the caller's changes that are not
  saved.
- OMEdit passes the whole text of the class's file (for a class in a one-file package: the text
  before, the class, the text after), so the classes of that file are seen as they are in
  OMEdit. It passes the directories of `getModelicaPath()` as `--library`.

Modifiers and redeclarations are not applied in the lookup.

## Use

- One thread at a time: the message is kept per library, not per thread.
- `mocoal_layout` returns when the layout is done. There is no progress and no cancel.
- Load the library once and keep it loaded. An implementation need not survive an unload.

## This implementation

Not part of the interface; what a caller of this implementation (Rust) has to know.

- `libmocoal.{so,dylib}`, `mocoal.dll` on Windows: one file, 1.5 MB on macOS arm64, no other
  libraries than the system's. On macOS its install name is `@rpath/libmocoal.dylib`.
- It also exports 3 symbols of the parser crates (`omrs_add_runtime_error`,
  `omrs_add_runtime_error_pos`, `omrs_runtime_abort`). They are not part of the interface.
- A panic inside `mocoal_layout` is caught: the result is `NULL`, the message
  `internal error, no layout`.
- Time: Modelica Standard Library 4.1.0, 2552 files, 1.1 to 1.5 s for all of them with the
  program `mocoal`.
- Licence: OSMC-PL 1.8 or AGPL v3, according to the recipient's choice.

## Test

[test_lib.c](test_lib.c) is a C program that uses only the four functions. It checks: a layout
with the defaults, a second layout of the result, options, a parse error, an unknown value, 9
words that are refused, a text without classes, words in double quotes, and a class that is not
there.

```bash
scripts/check.sh lib
```

Another build of the library: `CARGO_TARGET=<directory of the library> scripts/check.sh lib`.

The test `interface` in `src/capi.rs` checks the same rules from Rust.

## Open points

Not decided; each one would change the interface.

1. **Version of the interface.** No function gives it, and OMEdit does not call
   `mocoal_version`. A fifth function (`int mocoal_interface(void)`, 1 today) would let a caller
   refuse a library it does not know.
2. **Classes in memory.** The lookup reads files. A caller with classes that are not saved
   cannot pass them. Possible: a callback that returns the text of a class by name, or a second
   text argument.
3. **Options as text.** A word cannot contain a double quote. OMEdit does not put the `--class`
   name in quotes, so a quoted identifier with a space (`'my model'`) fails there.
4. **Messages.** No codes and no severity. Warnings and errors differ only by the `NULL`.
5. **Threads and cancel.** Calls from other threads are not tested. A layout cannot be stopped.
6. **Where the library is.** OMEdit looks at its setting `autoLayout/library`, the environment
   variable `MOCOAL`, then `OPENMODELICAHOME/lib/libmocoal.*`. How the library is shipped with
   OMEdit is not decided. In OMEdit the library was loaded on macOS arm64 only.
