#!/system/bin/sh
# Capability-checked, session-only governor changes. No chmod, frequency or thermal writes.
cpu_governor() {
  command=$1 policy=$2 target=$3
  printf '%s\n' "$policy" | grep -Eq '^policy[0-9]{1,3}$' || { echo "Invalid policy."; return 2; }
  path=/sys/devices/system/cpu/cpufreq/$policy
  [ -r "$path/scaling_governor" ] && [ -w "$path/scaling_governor" ] || { echo "Governor node is not accessible."; return 1; }
  boot=$(cat /proc/sys/kernel/random/boot_id) || return 1
  current=$(cat "$path/scaling_governor") || return 1
  printf '%s\n' "$current" | grep -Eq '^[a-z][a-z0-9_]{0,31}$' || return 1
  state=/data/adb/butterd
  umask 077
  mkdir -p "$state" || return 1
  chmod 0700 "$state" || return 1
  mkdir "$state/cpu.lock" 2>/dev/null || { echo "Another operation is active; no settings changed."; return 1; }
  trap 'rmdir /data/adb/butterd/cpu.lock 2>/dev/null' EXIT
  trap 'exit 1' HUP INT TERM
  saved=$state/$policy
  original=$current expected=$current saved_boot=$boot
  if [ -f "$saved" ]; then
    read -r saved_boot original expected < "$saved" || return 1
    if [ "$saved_boot" != "$boot" ]; then
      [ "$command" != restore ] || { echo "Saved values belong to an earlier boot; nothing restored."; return 1; }
      original=$current expected=$current
    elif [ "$expected" != "$current" ]; then
      echo "Another controller changed $policy; nothing overwritten."; return 1
    fi
  elif [ "$command" = restore ]; then
    echo "No saved governor for $policy."; return 1
  fi
  printf '%s\n' "$original" | grep -Eq '^[a-z][a-z0-9_]{0,31}$' || return 1
  if [ "$command" = restore ]; then target=$original
  else
    case "$target" in schedutil|ondemand|conservative|powersave|interactive) ;; *) echo "Unsupported governor choice."; return 2 ;; esac
  fi
  case " $(cat "$path/scaling_available_governors") " in *" $target "*) ;; *) echo "Governor is not exposed by this policy."; return 1 ;; esac
  # Retain the original before writing. Interrupted operations keep their journal.
  printf '%s %s %s\n' "$boot" "$original" "$target" > "$saved.tmp" && mv "$saved.tmp" "$saved" || return 1
  if ! printf '%s\n' "$target" > "$path/scaling_governor"; then
    if [ "$(cat "$path/scaling_governor")" = "$current" ]; then
      printf '%s %s %s\n' "$boot" "$original" "$current" > "$saved.tmp" && mv "$saved.tmp" "$saved"
    fi
    echo "Write failed; saved original retained."; return 1
  fi
  [ "$(cat "$path/scaling_governor")" = "$target" ] || { echo "Readback did not match; saved original retained."; return 1; }
  if [ "$command" = restore ]; then rm -f "$saved"; fi
  printf '%s governor verified: %s\n' "$policy" "$target"
}
