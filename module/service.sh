#!/system/bin/sh
MODDIR=${0%/*}
count=0
while [ "$(getprop sys.boot_completed)" != 1 ]; do
  [ "$count" -lt 120 ] || exit 0
  sleep 2
  count=$((count + 1))
done
umask 077
mkdir -p /data/adb/butterd || exit 1
# Only the user's explicit Automatic option enables tuning after reboot.
"$MODDIR/bin/butter" boot > /data/adb/butterd/boot.log 2>&1
