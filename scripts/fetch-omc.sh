#!/bin/sh
# Fetches the parser crates of OpenModelica's Rust omc into omc/, at the commit in omc/COMMIT.
# From the checkout in OPENMODELICA_SRC when that is set, else from GitHub.
set -eu

here=$(cd "$(dirname "$0")/.." && pwd)
commit=$(cat "$here/omc/COMMIT")
compiler=OMCompiler/Compiler
base=$compiler/OpenModelica.rs
crates="openmodelica_ast openmodelica_error openmodelica_wasi metamodelica metamodelica_derive"
# openmodelica_wasi embeds these with include_str!, by their path in the tree.
builtins="FrontEnd/ModelicaBuiltin.mo FrontEnd/MetaModelicaBuiltin.mo FrontEnd/AnnotationsBuiltin_1_x.mo
FrontEnd/AnnotationsBuiltin_2_x.mo FrontEnd/AnnotationsBuiltin_3_x.mo NFFrontEnd/NFModelicaBuiltin.mo"
url=${OPENMODELICA_URL:-https://github.com/OpenModelica/OpenModelica.git}

rm -rf "$here/omc/OMCompiler"
paths=""
for c in $crates; do
  paths="$paths $base/$c"
done
for b in $builtins; do
  paths="$paths $compiler/$b"
done

if [ -n "${OPENMODELICA_SRC:-}" ]; then
  src=$OPENMODELICA_SRC
else
  src=$(mktemp -d)
  trap 'rm -rf "$src"' EXIT
  git -C "$src" init -q
  git -C "$src" remote add origin "$url"
  git -C "$src" sparse-checkout set --no-cone $paths
  git -C "$src" fetch -q --depth 1 --filter=blob:none origin "$commit"
  git -C "$src" checkout -q FETCH_HEAD
fi

# shellcheck disable=SC2086
git -C "$src" archive "$commit" $paths | tar -x -C "$here/omc"
rm -rf "$here/omc/$base/openmodelica_ast/tests"
echo "omc/$base: $crates at $commit"
