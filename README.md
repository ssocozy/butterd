# Butterd

A monochrome device monitor for POCO X6 Pro / Redmi K70E (Duchamp), built with
Java and XML. Android 14 minimum, compile/target SDK 36. Android 17 compatibility
has not been verified. White, black and system themes are available.

## Screens

- **Overview:** net battery power, temperature, voltage, thermal status and session
  deep sleep. Shortcuts open the detail screens.
- **CPU:** online cores, per-policy governors and ROM frequency ranges. With explicit
  root access, choose a supported allowlisted dynamic/power-saving governor or
  restore the original saved for this boot. There is no automatic application at
  boot, core hotplug, voltage tuning or frequency-limit write.
- **Battery:** signed battery current, USB readings, requested charging-current
  limit, ROM charging-disable flag and driver-reported primary power path, where
  readable. These flags do not prove physical bypass charging.
- **GPU:** readable driver frequency. GPU tuning remains ROM-managed.
- **Device:** hardware and access information, plus shortcuts to compatible
  XiaomiParts screens. Butterd does not edit XiaomiParts private preferences.
- **Settings:** appearance and a permission-gated, quiet live notification with Stop.

Battery power is net power **at the battery**, not adapter wattage. Missing data
stays unavailable. Deep sleep estimates device suspend over the app-process
session, not screen-off time or Doze. Polling does not hold a wake lock; notification
values remain the latest sample while the CPU is suspended.

## Record a charging test

1. Open **Battery → Record diagnostics** and review what will be recorded.
2. Allow notifications. Optional root access on Device unlocks more sensor reads.
3. Open XiaomiParts and reproduce the lower-current/bypass sequence there.
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

Only an explicit CPU governor change writes a kernel setting. It is restricted to
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
Each push runs **one job**: unit tests, lint and APK assembly. No emulators run.
Download `butterd-debug` from a successful run and extract the installable APK.
Development APKs use runner debug keys, which can change between runs and require
uninstall/reinstall. Stable release signing must be configured before distributing
updateable production builds.

Physical sensor accuracy, root managers, charging behavior and frame pacing must
be checked on the phone. See [architecture](docs/architecture.md),
[source findings](docs/device-research.md), and [device checks](docs/device-validation.md).
