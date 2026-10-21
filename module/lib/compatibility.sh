#!/system/bin/sh
select_identity() {
  case "$1" in bluejay|panther|cheetah|lynx|shiba|husky|akita|tokay|caiman|komodo|tegu) ;; *) echo "Invalid Pixel model."; return 2 ;; esac
  profile=$MODDIR/compatibility/profiles/$1.prop
  [ -f "$profile" ] || { echo "No bundled profile is available for this model."; return 1; }
  grep -Eq '^FINGERPRINT=google/[a-z0-9_]+/[a-z0-9]+:CANARY/[A-Za-z0-9._-]+/[0-9]+:user/release-keys$' "$profile" || return 1
  # Journal the selected model in the same atomically replaced config file.
  umask 077
  temporary=$(mktemp "$MODDIR/compatibility/selected.XXXXXX") || return 1
  if { cat "$profile"; printf 'selectedModel=%s\n' "$1"; } > "$temporary"; then
    mv -f "$temporary" "$MODDIR/compatibility/pif.prop" || { rm -f "$temporary"; return 1; }
  else rm -f "$temporary"; return 1; fi
  echo "Pixel profile selected. Restart the affected apps or reboot to apply."
}
