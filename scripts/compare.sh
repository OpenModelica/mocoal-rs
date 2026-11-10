#!/bin/sh
# Lay out a copy of a library with two mocoal programs and compare the files and the reports.
# usage: scripts/compare.sh MOCOAL_A MOCOAL_B LIBRARY_DIR WORK_DIR [mocoal options]
set -u
a=$1; b=$2; lib=$3; work=$4; shift 4
# The programs run in the work directory: make their paths absolute.
case $a in /*) ;; */*) a=$PWD/$a ;; esac
case $b in /*) ;; */*) b=$PWD/$b ;; esac
name=$(basename "$lib")
rm -rf "$work/a" "$work/b"
mkdir -p "$work/a" "$work/b"
cp -R "$lib" "$work/a/$name"
cp -R "$lib" "$work/b/$name"
(cd "$work/a" && "$a" -i --svg svg "$@" "$name" 2> ../a.log); sa=$?
(cd "$work/b" && "$b" -i --svg svg "$@" "$name" 2> ../b.log); sb=$?
differing=$(diff -rq "$work/a" "$work/b" | wc -l | tr -d ' ')
reports=same
cmp -s "$work/a.log" "$work/b.log" || reports=DIFFERENT
echo "options '$*': status $sa $sb, $(tail -1 "$work/a.log"), $(ls "$work/a/svg" | wc -l | tr -d ' ') SVG, files that differ: $differing, reports: $reports"
[ "$sa" = "$sb" ] && [ "$differing" = 0 ] && [ $reports = same ]
