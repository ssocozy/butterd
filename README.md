# Butterd

A monochrome device monitor and manual controller for POCO X6 Pro / Redmi K70E (Duchamp), built with
Java and XML. Android 14 minimum, compile/target SDK 36. Version 0.4 adds the supplied butter logo,
short page/press/splash transitions, and separate Display and Kernel screens. Android 17 compatibility
has not been verified. White, black and system themes are available.

## Screens

- **Overview:** net battery power, temperature, voltage, thermal status and session
  deep sleep. Shortcuts open the detail screens.
- **CPU:** online cores, per-policy governors and ROM frequency ranges. With explicit
  root access, choose a supported allowlisted dynamic/power-saving governor or
  restore the original saved for this boot. There is no automatic application at
  boot, core hotplug, voltage tuning or frequency-limit write.
- **Battery:** Android health status/cycles, driver full/design capacity and a
  capacity-retention estimate when provided. Signed current, USB readings, requested charging-current
  limit, ROM charging-disable flag and driver-reported primary power path, where
  readable. These flags do not prove physical bypass charging.
- **GPU:** readable driver frequency and ROM GPU DVFS parameters. GPU tuning is not implemented yet.
- **Display:** Android-reported Hz and root-controlled global refresh preferences
  selected inside Butterd, with supported-mode validation, saved originals,
  readback and Restore. Readable HTSR mode remains a sensor. Per-app refresh
  overrides are not implemented.
- **Kernel:** current/available/allowed TCP congestion algorithms and default qdisc.
  These are capabilities, not network tuning recommendations.
- **Device:** hardware, access information and navigation to Butterd controls.
  Butterd does not depend on XiaomiParts or open its controls.
- **Settings:** appearance and a permission-gated, quiet live notification with Stop.

Battery power is net power **at the battery**, not adapter wattage.
An unplugged nonzero current sample calibrates vendor sign conventions for the
current app process. Until then, the Android-reported sign is used. Connected
chargers can still coincide with negative net battery power. Diagnostic logs keep
the raw current and calibration state. Missing optional sensors are grouped behind
an access explanation. Deep sleep estimates device suspend over the app-process
session, not screen-off time or Doze. Polling does not hold a wake lock; notification
values remain the latest sample while the CPU is suspended.

## Record a charging test

1. Open **Battery → Record diagnostics** and review what will be recorded.
2. Allow notifications. Optional root access on Device unlocks more sensor reads.
3. Reproduce the charging behavior you want to diagnose. Butterd does not
   implement charging bypass yet.
4. Return to Battery and add a marker after each change.
5. Stop recording, then **Share recording** to send the `.jsonl` file yourself.

Recordings are opt-in and private until shared. They contain sensor readings,
CPU state, model and software versions, not serial numbers, IMEI, account data,
app lists or logcat. Maximum 30 minutes or 2 MB; the latest two sessions are kept.
Three export snapshots are retained in app cache. Sampling is approximately every
three seconds during recording, so short transitions may be missed. Root readings
are cached up to fifteen seconds otherwise. Sample age is included in reports.
Stopping the notification stops recording; stopping recording alone leaves the
live monitor enabled until you turn it off. No recording restarts after process
termination or reboot. There is no network permission or automatic upload.

## Control boundaries

Only explicit CPU governor changes write kernel settings. Display controls write
Android minimum/peak refresh preferences for the current user; these preferences
can persist across closing the app or rebooting until restored. It is restricted to
validated CPUFreq policy paths and supported allowlisted governors. Originals are
saved before writes, readback is checked, and restoration rejects an unexpected
external governor change instead of overwriting it. This check reduces races; it
is not a lock shared with ROM services. No setting is repeatedly forced.

Governor changes may persist after closing the app until restored or rebooted.
“Stop using root” stops privileged access; it does not restore a governor. Restore
before revoking access if desired. Reboot returns to ROM initialization. Nothing
is applied during boot. Device/kernel bugs and competing controllers still need
physical testing; zero-risk or guaranteed performance claims are not made.

Charging limits, thermal protections, CPU/GPU frequency limits, core online state,
partitions and SELinux are never written. There is no root module or boot receiver.
Charging bypass replacement remains deferred until the recorded hardware results
support a verified transition design owned by the ROM controller.

## Build and install

All compilation runs in [GitHub Actions](https://github.com/ssocozy/butterd/actions/workflows/android.yml).
Builds are **manual only** while development is active. Open Actions → Android →
Run workflow when ready. The single job runs unit tests, lint and APK assembly;
pushes and pull requests do not start it. No emulators run.
Download `butterd-debug.apk` directly from the latest debug [prerelease](https://github.com/ssocozy/butterd/releases).
The Actions artifact remains available as a ZIP fallback.
Development APKs use runner debug keys, which can change between runs and require
uninstall/reinstall. Stable release signing must be configured before distributing
updateable production builds.

Physical sensor accuracy, root managers, charging behavior and frame pacing need
verification on the phone. CPU/GPU frequency presets, charging and thermal
overrides, and independent per-app refresh/touch control are not implemented.

## Code organization

`core/root` owns bounded worker-only root process transport and read inventory.
`feature/cpu` owns validated policies and manual governor changes.
`feature/display` owns preference validation, transition journals, application,
restoration and the display panel. `feature/kernel` owns the fixed read inventory.
Other legacy panels still need extraction; this is an incremental refactor.

Display restoration stops when preferences no longer match the last journaled
transition. This reduces conflicts; it is not a lock shared with ROM services.
Readback confirms preference storage, not that Android must display the selected
rate. No preferences are reasserted on a timer or at boot. Restore before clearing
app data or uninstalling if you want to retain the original values.

The interface uses open reading sections, expandable technical details and explicit preview-and-apply dialogs for supported CPU governors and display preferences. Root permission is checked independently from optional sensor reads.
