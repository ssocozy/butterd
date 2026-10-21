#!/system/bin/sh
[ "$BOOTMODE" = true ] || abort "Install from KernelSU or Magisk while Android is running."
[ "$ARCH" = arm64 ] || abort "Butterd requires arm64 Duchamp."
[ "$API" -ge 34 ] || abort "Butterd requires Android 14 or newer."
case "$(getprop ro.product.device):$(getprop ro.product.vendor.device)" in
  duchamp:*|*:duchamp) ;;
  *) abort "This package supports Duchamp only." ;;
esac
for conflict in /data/adb/modules/*; do
  [ -d "$conflict" ] || continue
  [ -f "$conflict/disable" ] && continue
  [ -f "$conflict/remove" ] && continue
  case "${conflict##*/}" in
    butterd) continue ;;
    playintegrityfix|playintegrityfork|encore|duchamp_essential|duchamp-essential)
      abort "Disable the conflicting module ${conflict##*/} before installing Butterd." ;;
  esac
done
[ -f "$MODPATH/bin/butter-engine" ] || abort "The compiled Rust engine is missing."
[ -f "$MODPATH/zygisk/arm64-v8a.so" ] || abort "The compiled compatibility library is missing."
# Fetch metadata during installation; an offline install keeps bundled profiles.
MODDIR=$MODPATH
. "$MODPATH/lib/compatibility.sh"
existing=/data/adb/modules/butterd/compatibility/pif.prop
preserved=0
preserve_identity "$existing" || preserved=$?
if [ "$preserved" -eq 2 ]; then
  abort "Unable to preserve the existing Pixel profile; installation stopped."
fi
if [ "$preserved" -eq 0 ]; then
  ui_print "- Keeping your existing Pixel identity and compatibility settings."
else
  if [ -f "$existing" ] && [ ! -L "$existing" ]; then
    existing_size=$(wc -c < "$existing")
    if [ "$existing_size" -le 16384 ]; then
      cp "$existing" "$MODPATH/compatibility/pif.previous.prop" || abort "Unable to back up the previous Pixel profile."
      ui_print "- Previous profile was invalid; backed up as compatibility/pif.previous.prop."
    fi
  fi
  selected=$(random_identity) || abort "Unable to select a random Pixel profile."
  ui_print "- Refreshing Pixel 6a–9a profiles (offline fallback included)."
  CLASSPATH="$MODPATH/bin/catalog.dex" /system/bin/app_process /system/bin dev.butterd.module.PixelCatalog "$MODPATH/compatibility/profiles" || ui_print "- Network unavailable; keeping bundled profiles."
  select_identity "$selected" || abort "No usable Pixel identity is available."
fi
# Leave webroot permissions/context to the WebUI host.
for directory in bin lib compatibility zygisk; do
  set_perm_recursive "$MODPATH/$directory" 0 0 0755 0644
done
set_perm "$MODPATH/compatibility/pif.prop" 0 0 0600
[ ! -f "$MODPATH/compatibility/pif.previous.prop" ] || set_perm "$MODPATH/compatibility/pif.previous.prop" 0 0 0600
set_perm "$MODPATH/bin/butterctl" 0 0 0755
set_perm "$MODPATH/bin/butter" 0 0 0755
set_perm "$MODPATH/bin/butter-engine" 0 0 0755
set_perm "$MODPATH/service.sh" 0 0 0755
set_perm "$MODPATH/action.sh" 0 0 0755
ui_print "- Automatic tuning and advanced overrides stay off until you enable them."
ui_print "- Compatibility requires enabled Zygisk (or a compatible KernelSU Zygisk provider)."
ui_print "- Three compatibility options are included; no integrity verdict is guaranteed."
