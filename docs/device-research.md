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

## XiaomiParts integration boundary

The inspected manifest exports PartsActivity and thermal.ThermalProfileActivity.
TurboChargingActivity and PerAppThermalActivity are not exported. Thermal services
are private, and the charge service has no Binder API (onBind returns null).
Per-app configuration lives in XiaomiParts private SharedPreferences. No public
profile-control provider or callable command receiver was found in this revision.
Butterd opens the two exported screens using explicit components, checks their
availability/export status/permissions, and handles denied or missing activities.
It does not edit XiaomiParts preferences or impersonate its system UID.
Embedded controls need a separate permission-protected ROM-side interface with
state verification and ownership coordination; direct sysfs writes are not an
API to XiaomiParts.

## Bypass entry sequencing audit

User observation: bypass works reliably only after selecting a lower charging
preset (reported as 20 W or 37 W on the installed ROM). This is a hardware
observation to reproduce, not a source-proven requirement or universal limit.
The inspected device tree labels its presets 18 W / 30 W / 67 W and writes
4,800,000 / 7,000,000 / 22,000,000 to constant_charge_current. These are current
requests, not adapter wattage contracts; the installed build's mapping may differ.

ChargeController.setBypass first changes current_cmd and then immediately calls
apply(), which requests zero current when the software disable flag is set.
There is no preparation phase, settling confirmation or rollback. Failure to read
the flag is treated as disabled, allowing the normal current request to be applied
after an unsuccessful read. Successful flag readback still does not verify the
physical power path.

In the linked kernel sources, current_cmd toggles primary charger enable and
cmd_discharging. It does not explicitly coordinate the separate PD charge-pump
state machine. The zero SIC current vote reduces target_fcc below its entry
threshold, causing HOLD/EXIT. PD_PM_STATE_EXIT disables the charge pumps,
renegotiates adapter output and restores the primary power path when input is
not suspended. Therefore entry involves asynchronous transitions between separate
controllers. A lower preset could affect that transition, but the exact reason
for the user's workaround is not established by static inspection.

The driver's internal bypass_enable selects a charge-pump conversion mode; that
name alone does not establish a user-facing battery bypass control. No dedicated,
validated battery-bypass interface was identified in this audit.

A fix should be owned by the ROM controller, with distinct preparing, verified
active, restoring and failed states. It must verify power-path/charging state and
battery current, preserve thermal protections, bound transitions and recover
partial failures. Selecting an arbitrary wattage and sleeping a fixed number of
seconds is not sufficient validation. No charging writes or production fix have
been added to Butterd.

## CPU settings and cores

CPUFreq policy governors and frequency limits affect CPUs sharing that policy;
they do not select how many cores remain online. This tree separately configures
cpusets (foreground/top-app 0–7, background 0–3) and permissions on cpu*/online.
Cpuset membership controls task placement, not physical core power state.
Manual core hotplug is outside Butterd's initial safety boundary.
