# Duchamp source research

Device tree: https://github.com/DuchampReborn/device_xiaomi_duchamp
Inspected branch: lineage-24.0
Commit: bb164ce7c24b6b56d471bad3216c8769643d66cc

Linked kernel module sources: https://github.com/mt6897-devs/android_kernel_device_modules_6.1
Inspected branch: lineage-23.2
Commit: 324c04a3dd1e2e5cc25ac0389923ad27e7165c78
The link does not prove that this revision matches a particular installed ROM's
prebuilt kernel. Runtime capabilities must still be checked.

## Confirmed integration points

- XiaomiParts thermal profile uses `/sys/class/thermal/thermal_message/sconfig`.
  IDs: 50 Balanced, 18 Performance, 700 Battery saver, 19 Gaming. Butterd reads it.
  Per-app and screen-state automation already exist in the ROM.
- `configs/powerhint.json` gives the ROM service direct control of memory frequency
  and CPU cluster limits. Butterd does not compete with it.
- `configs/thermal_info_config.json` maps CPU, GPU, NPU and SOC to `soc_max`.
  Butterd labels that reading SoC / CPU and includes its sensor name, rather than
  claiming separate CPU and GPU temperature sensors.
- `init/init.project.rc` makes GED `current_freqency` and `gpu_utilization` readable.
  The misspelling is the actual kernel interface. `ged_hal.c` emits current GPU
  OPP index and frequency as two unsigned integers; Butterd parses that format.
- XiaomiParts ChargeController controls battery `constant_charge_current` and
  `/proc/mtk_battery_cmd/current_cmd`. TurboChargingService reapplies current
  preferences. Butterd writes neither interface.
- The current_cmd kernel readback is usb_unlimited and cmd_discharging. Its write
  handler toggles primary charging; a set flag does not verify adapter-to-system
  power routing or near-zero physical battery current.
- The battery current property feeds charger_manager_set_sic_current and SIC_VOTER.
  Requesting zero current can put the PD charge-pump state machine into HOLD.
- The tree's thermal .conf files are binary. Their charging rules were not decoded.
- `thermal_remove` occurs in linked kernel charging code, not the inspected device
  tree. It substitutes thermal level zero, while separate JEITA and connector
  protection paths exist. This is not evidence of a safe, universal thermal bypass.

## Audit findings affecting design

The existing bypass controller catches write failures without returning success,
uses two independent writes without rollback, and equates a command flag with
actual bypass. Its screen can remain stale after service-side unplug handling.
Butterd does not reproduce those behaviors or offer a bypass toggle in 0.1.
