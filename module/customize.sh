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
[ -f "$MODPATH/zygisk/arm64-v8a.so" ] || abort "The compiled compatibility library is missing."
# Fetch metadata during installation; an offline install keeps bundled profiles.
selected=bluejay
if [ -f /data/adb/modules/butterd/compatibility/pif.prop ]; then
  previous=$(sed -n 's/^selectedModel=//p' /data/adb/modules/butterd/compatibility/pif.prop)
  case "$previous" in bluejay|panther|cheetah|lynx|shiba|husky|akita|tokay|caiman|komodo|tegu) selected=$previous ;; esac
fi
ui_print "- Refreshing Pixel 6a–9a profiles (offline fallback included)."
CLASSPATH="$MODPATH/bin/catalog.dex" /system/bin/app_process /system/bin dev.butterd.module.PixelCatalog "$MODPATH/compatibility/profiles" || ui_print "- Network unavailable; keeping bundled profiles."
MODDIR=$MODPATH
. "$MODPATH/lib/compatibility.sh"
select_identity "$selected" || abort "No usable Pixel identity is available."
# Leave webroot permissions/context to the WebUI host.
for directory in bin lib compatibility zygisk; do
  set_perm_recursive "$MODPATH/$directory" 0 0 0755 0644
done
set_perm "$MODPATH/bin/butterctl" 0 0 0755
set_perm "$MODPATH/service.sh" 0 0 0755
set_perm "$MODPATH/action.sh" 0 0 0755
ui_print "- No system overlay, thermal overrides or boot performance writes."
ui_print "- Compatibility requires enabled Zygisk (or a compatible KernelSU Zygisk provider)."
ui_print "- Three compatibility options are included; no integrity verdict is guaranteed."
