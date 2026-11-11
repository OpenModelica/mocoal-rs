#!/bin/sh
# Check what `cargo build --release` built.
#
#   scripts/check.sh [exe|lib|sync|all]        default: all
#
# exe:  run mocoal on test/models/Chain.mo.
# lib:  call libmocoal from the C program interface/test_lib.c.
# sync: compare interface/ and examples/ with the files of ModelicaComponentAutolayout.jl, where
#       the interface is kept (MOCOAL_JL names its checkout, default ../ModelicaComponentAutolayout.jl).
# all:  exe, lib, and sync when that checkout is there.
#
# CARGO_TARGET names the directory of the build (default target/release), CC the C compiler
# (default cc, gcc on Windows).
set -u
cd "$(dirname "$0")/.." || exit 1
what=${1:-all}
target=${CARGO_TARGET:-target/release}
julia=${MOCOAL_JL:-../ModelicaComponentAutolayout.jl}
out=build
model=test/models/Chain.mo
mkdir -p "$out"

check_exe() {
  exe=$target/mocoal
  "$exe" --version || return 1
  "$exe" -o "$out/Chain.mo" "$model" || return 1
  [ "$(grep -c 'Placement(transformation(origin=' "$out/Chain.mo")" = 3 ] || { echo "mocoal: expected 3 placements"; return 1; }
  [ "$(grep -c 'Line(points=' "$out/Chain.mo")" = 2 ] || { echo "mocoal: expected 2 lines"; return 1; }
  "$exe" -q -o "$out/Chain2.mo" "$out/Chain.mo" || return 1
  cmp "$out/Chain.mo" "$out/Chain2.mo" || { echo "mocoal: the second layout differs"; return 1; }
  "$exe" --layout circle "$model" 2> /dev/null
  [ $? = 2 ] || { echo "mocoal: expected exit status 2"; return 1; }
  echo "mocoal: ok"
}

check_lib() {
  lib=
  for name in libmocoal.so libmocoal.dylib mocoal.dll; do
    [ -f "$target/$name" ] && lib=$target/$name
  done
  [ -n "$lib" ] || { echo "libmocoal: not built"; return 1; }
  dir=$(cd "$target" && pwd)
  case $lib in
    # Windows finds a DLL next to the program.
    *.dll) cp "$lib" "$out/" && ${CC:-gcc} -o "$out/test_lib" interface/test_lib.c "$out/mocoal.dll" || return 1 ;;
    *) ${CC:-cc} -o "$out/test_lib" interface/test_lib.c -L"$dir" -lmocoal -Wl,-rpath,"$dir" || return 1 ;;
  esac
  "$out/test_lib" "$model" || return 1
  echo "libmocoal: ok"
}

check_sync() {
  [ -d "$julia" ] || { echo "interface: no checkout at $julia"; return 1; }
  cmp interface/INTERFACE.md "$julia/INTERFACE.md" || return 1
  cmp interface/test_lib.c "$julia/juliac/test_lib.c" || return 1
  cmp "$model" "$julia/$model" || return 1
  diff -r examples "$julia/examples" || return 1
  echo "interface: the copies are those of $julia"
}

case $what in
  exe) check_exe ;;
  lib) check_lib ;;
  sync) check_sync ;;
  all)
    status=0
    check_exe || status=1
    check_lib || status=1
    if [ -d "$julia" ]; then check_sync || status=1; else echo "interface: not compared, no checkout at $julia"; fi
    exit $status
    ;;
  *) echo "usage: scripts/check.sh [exe|lib|sync|all]"; exit 2 ;;
esac
