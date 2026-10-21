#!/system/bin/sh
MODDIR=${0%/*}
# Restore only settings still owned by butter; journal conflicts remain visible.
"$MODDIR/bin/butter" shutdown > /data/adb/butterd/uninstall.log 2>&1
