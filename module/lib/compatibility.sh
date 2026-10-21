#!/system/bin/sh
random_identity() {
  # Reject the incomplete range so all eleven models have equal probability.
  while :; do
    random_byte=$(od -An -N1 -tu1 /dev/urandom) || return 1
    case "$random_byte" in *[!0-9\ ]*|'') return 1 ;; esac
    [ "$random_byte" -lt 253 ] && break
  done
  set -- bluejay panther cheetah lynx shiba husky akita tokay caiman komodo tegu
  shift "$((random_byte % 11))"
  printf '%s\n' "$1"
}

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
  if [ "${2:-}" = restart ]; then
    restart_compatibility || { echo "Profile saved, but a Google process could not be restarted. Close Play Store and retry."; return 1; }
    echo "Pixel profile applied. Google integrity and Play Store processes will reload it; no phone reboot needed."
  else
    echo "Pixel profile selected. It will load after installation and reboot."
  fi
}

restart_compatibility() {
  restart_failed=0
  for target in com.google.android.gms.unstable com.android.vending; do
    process_ids=$(pidof "$target" 2>/dev/null) || process_ids=
    process_count=0
    for process_id in $process_ids; do
      case "$process_id" in ''|*[!0-9]*) restart_failed=1; continue ;; esac
      [ "${#process_id}" -le 8 ] && [ "$process_id" -gt 1 ] || { restart_failed=1; continue; }
      process_count=$((process_count + 1))
      [ "$process_count" -le 16 ] || { restart_failed=1; break; }
      # Restart only upstream's two target processes; never clear Google application data.
      if ! kill -TERM "$process_id" 2>/dev/null; then
        kill -0 "$process_id" 2>/dev/null && restart_failed=1
      fi
    done
  done
  [ "$restart_failed" -eq 0 ]
}

preserve_identity() {
  existing_profile=$1
  [ -f "$existing_profile" ] && [ ! -L "$existing_profile" ] || return 1
  existing_size=$(wc -c < "$existing_profile") || return 2
  [ "$existing_size" -gt 0 ] && [ "$existing_size" -le 16384 ] || return 1
  # Validate a normalized view, then copy the original bytes unchanged.
  existing_values=$(sed -e 's/#.*//' -e 's/\r$//' -e 's/^[[:space:]]*//' -e 's/[[:space:]]*=[[:space:]]*/=/' -e 's/[[:space:]]*$//' "$existing_profile") || return 2
  [ "$(printf '%s\n' "$existing_values" | grep -c '^FINGERPRINT=')" -eq 1 ] || return 1
  printf '%s\n' "$existing_values" | grep -Eq '^FINGERPRINT=[A-Za-z0-9._-]+/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+:[A-Za-z0-9._-]+/[A-Za-z0-9._-]+/[A-Za-z0-9._-]+:[A-Za-z0-9._-]+/[A-Za-z0-9._-]+$' || return 1
  [ "$(printf '%s\n' "$existing_values" | grep -c '^MODEL=')" -eq 1 ] || return 1
  # Android's regex engine rejects repetition bounds above 255, even for short models.
  # Check length separately so valid identities behave identically on Android and CI.
  existing_model=$(printf '%s\n' "$existing_values" | sed -n 's/^MODEL=//p')
  [ "${#existing_model}" -ge 1 ] && [ "${#existing_model}" -le 256 ] || return 1
  printf '%s\n' "$existing_model" | LC_ALL=C grep -Eq '^[ -~]+$' || return 1
  for existing_flag in spoofBuild spoofVendingBuild spoofProps; do
    existing_count=$(printf '%s\n' "$existing_values" | grep -c "^$existing_flag=" || true)
    [ "$existing_count" -le 1 ] || return 1
    if [ "$existing_count" -eq 1 ]; then
      printf '%s\n' "$existing_values" | grep -Eq "^$existing_flag=(true|false|0|1)$" || return 1
    fi
  done
  umask 077
  preserved_temporary=$(mktemp "$MODDIR/compatibility/preserved.XXXXXX") || return 2
  if ! cat "$existing_profile" > "$preserved_temporary" || ! mv -f "$preserved_temporary" "$MODDIR/compatibility/pif.prop"; then
    rm -f "$preserved_temporary"; return 2
  fi
  return 0
}
