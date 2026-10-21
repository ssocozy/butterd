#!/bin/sh
# Host-only mocked process commands; run exclusively in Actions.
set -eu
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
MODDIR=$scratch/module
mkdir -p "$MODDIR/compatibility/profiles"
cp module/compatibility/profiles/tegu.prop "$MODDIR/compatibility/profiles/tegu.prop"
. module/lib/compatibility.sh
pidof() {
  printf 'lookup %s\n' "$1" >> "$scratch/calls"
  case "$1" in com.google.android.gms.unstable) echo 12345 ;; com.android.vending) echo 23456 ;; *) return 1 ;; esac
}
kill() { printf 'signal %s %s\n' "$1" "$2" >> "$scratch/calls"; }
select_identity tegu
[ ! -e "$scratch/calls" ]
select_identity tegu restart
[ "$(grep -c '^lookup ' "$scratch/calls")" -eq 2 ]
grep -q '^signal -TERM 12345$' "$scratch/calls"
grep -q '^signal -TERM 23456$' "$scratch/calls"
grep -q '^selectedModel=tegu$' "$MODDIR/compatibility/pif.prop"
rm "$scratch/calls"
pidof() { printf '%s\n' '-1 0 1 invalid'; }
if restart_compatibility; then echo 'Unsafe PID accepted'; exit 1; fi
[ ! -e "$scratch/calls" ]
echo 'Atomic profile selection and targeted process-restart fixtures passed'
