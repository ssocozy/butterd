#!/system/bin/sh
MODDIR=${0%/*}
# Diagnostics only. No perpetual polling, boot tuning or Google process killing.
count=0
while [ "$(getprop sys.boot_completed)" != 1 ]; do
  [ "$count" -lt 120 ] || exit 0
  sleep 2
  count=$((count + 1))
done
umask 077
mkdir -p /data/adb/butterd || exit 1
"$MODDIR/bin/butterctl" status > /data/adb/butterd/status.txt 2>&1
