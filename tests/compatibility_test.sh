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

# Upgrades retain the full identity and user settings, including comments and disabled options.
cp "$MODDIR/compatibility/pif.prop" "$scratch/existing.prop"
printf '# Keep my working identity\n' >> "$scratch/existing.prop"
sed 's/spoofBuild=true/spoofBuild=false/' "$scratch/existing.prop" > "$scratch/changed.prop"
preserve_identity "$scratch/changed.prop"
cmp "$scratch/changed.prop" "$MODDIR/compatibility/pif.prop"
[ ! -e "$scratch/calls" ]
cp "$MODDIR/compatibility/pif.prop" "$scratch/saved.prop"
printf 'FINGERPRINT=broken\nMODEL=Invalid\n' > "$scratch/invalid.prop"
if preserve_identity "$scratch/invalid.prop"; then echo 'Malformed identity preserved'; exit 1; fi
cmp "$scratch/saved.prop" "$MODDIR/compatibility/pif.prop"
ln -s "$scratch/changed.prop" "$scratch/link.prop"
if preserve_identity "$scratch/link.prop"; then echo 'Symlink identity accepted'; exit 1; fi
if preserve_identity "$scratch/missing.prop"; then echo 'Missing profile accepted'; exit 1; fi
printf 'spoofProps=true\n' >> "$scratch/changed.prop"
if preserve_identity "$scratch/changed.prop"; then echo 'Duplicate flag accepted'; exit 1; fi
echo 'Upgrade profile preservation fixtures passed'

# Reproduce Android's regex repetition limit rather than relying on GNU grep alone.
grep() {
  for argument do
    case "$argument" in *'{1,256}'*) echo 'Android regex: repetition bound exceeds 255' >&2; return 2 ;; esac
  done
  command grep "$@"
}
cat > "$scratch/pixel8a.prop" <<'PROFILE'
FINGERPRINT=google/akita_beta/akita:CANARY/ZP11.260918.007/16484274:user/release-keys
MANUFACTURER=Google
MODEL=Pixel 8a
SECURITY_PATCH=2026-10-05
DEVICE_INITIAL_SDK_INT=34
spoofBuild=true
spoofVendingBuild=true
spoofProps=true
selectedModel=akita
PROFILE
preserve_identity "$scratch/pixel8a.prop"
cmp "$scratch/pixel8a.prop" "$MODDIR/compatibility/pif.prop"
for model_length in 0 256 257; do
  model=$(awk -v count="$model_length" 'BEGIN {for (i=0;i<count;i++) printf "A"}')
  sed "s/^MODEL=.*/MODEL=$model/" "$scratch/pixel8a.prop" > "$scratch/length.prop"
  if [ "$model_length" -eq 256 ]; then
    preserve_identity "$scratch/length.prop"
    cmp "$scratch/length.prop" "$MODDIR/compatibility/pif.prop"
  elif preserve_identity "$scratch/length.prop"; then
    echo 'Invalid model length accepted'; exit 1
  fi
done
sed 's/^MODEL=.*/MODEL=Pixel\t8a/' "$scratch/pixel8a.prop" > "$scratch/control.prop"
if preserve_identity "$scratch/control.prop"; then echo 'Control character accepted'; exit 1; fi
printf 'MODEL=Pixel 8a\n' >> "$scratch/pixel8a.prop"
if preserve_identity "$scratch/pixel8a.prop"; then echo 'Duplicate model accepted'; exit 1; fi
echo 'Android-compatible model validation fixtures passed'
