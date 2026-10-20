# Architecture and safety decisions

## Process and permissions

One Application-owned repository samples on a background HandlerThread. UI and
notification observers share it. Sampling stops when there are no observers;
there is no independent polling loop per screen. Android APIs provide standard
battery telemetry and thermal severity. Readable sysfs supplies optional sensors.
Root is requested only from the user's access button and is not persisted across
process restarts. A failed root read clears privileged data and falls back to
standard telemetry. Applied CPU governors are not automatically restored on root
revocation; a saved original remains available for explicit restoration. su receives a fixed command, never paths or commands from an
Intent, preference, network, or text field. It only reads bounded sensor files.
The root process has a timeout and is destroyed afterward. Permissions granted
in an external root manager are managed by that manager, not revoked by the app's
“Stop using root” button.

A monitoring session uses SystemClock.elapsedRealtime minus uptimeMillis. This
measures time in suspend over the process session. No wake lock is taken. Screen
on/off is not used as a proxy. Polls run about every three seconds while interactive
and fifteen seconds otherwise, and do not wake a suspended CPU. Root inventory
is cached for up to fifteen seconds; the screen therefore combines latest samples,
not an atomic hardware capture. The graph is an in-memory fixed ring buffer.

## Notifications

Only an Activity in the foreground starts the monitor after a user gesture.
Android 13+ notification permission is requested in that context. Both app and
channel blocking are checked. A specialUse foreground service declares its actual
purpose and is not misclassified as dataSync, health or connectedDevice. It posts
immediately, then updates quietly at most once per fifteen seconds while executing.
There is no auto-start on boot or process death: START_NOT_STICKY is intentional.
Stopping the monitor removes its observer and notification. Force stop and reboot
leave nothing to restore without reapplying device settings. A manual CPU governor change can remain until
restored or until ROM boot initialization resets it.

## Interface

Apple HIG Materials influenced the distinction between content and a single
floating navigation layer. HIG Accessibility informed text hierarchy, labeled
controls, contrast and transparency choice. Native Android permission dialogs,
snackbars, insets and 48dp-or-larger controls preserve platform behavior. White and
black surfaces remain monochrome. The initial navigation uses translucent tint,
not live bitmap capture; true backdrop blur is deferred until profiling justifies
it. Java/XML is the chosen implementation, not a claim that the language alone
improves frame pacing. IO never runs in render callbacks. No permanent animation
loop or bitmap effect runs while scrolling.

## References read

- https://developer.apple.com/design/human-interface-guidelines/materials
  (Markdown content at https://developer.apple.com/tutorials/data/design/human-interface-guidelines/materials.md)
- https://developer.apple.com/design/human-interface-guidelines/accessibility
  (Markdown content at https://developer.apple.com/tutorials/data/design/human-interface-guidelines/accessibility.md)
- https://developer.android.com/reference/android/os/BatteryManager
- https://developer.android.com/reference/android/os/SystemClock
- https://developer.android.com/develop/ui/views/notifications/notification-permission
- https://developer.android.com/develop/background-work/services/fgs/service-types
- https://developer.android.com/about/versions/16/setup-sdk

## Future device control gates

Thermal or charging controls need a separate, reviewed design with verified driver
semantics, readback, partial-failure handling, ownership coordination with
XiaomiParts and hardware testing. There is no “enable all writes with root” switch.
No unsupported safety guarantees such as “cannot brick” are made. CPU governor
selection is the sole explicit, bounded write path; other tuning remains deferred.

## Context panels, CPU changes and diagnostics

CPU, Battery and GPU detail screens inflate lazily on first visit. Overview keeps
summary readings and contextual shortcuts. The CPU screen is the only write-capable
surface: explicit confirmation may change one CPUFreq scaling_governor to a
supported allowlisted governor. Performance and userspace governors are excluded
from selection. Frequency limits, core online state, thermal and charging nodes
remain untouched. The worker saves original/expected governor and boot UUID in a
private journal before writing, validates paths and values, compares the current
governor before a write and checks readback afterward. Restore refuses to overwrite
an unexpected external state. This is optimistic coordination, not proof that no
other controller can race a change. No background reassertion or boot application
exists. Closing the app does not automatically restore an applied governor.

Diagnostics use a bounded private JSONL recorder on the existing worker thread.
A recording requires the explicitly started foreground notification. The service
must acknowledge startForegroundService before rejecting a denied-permission
request; stopping before that acknowledgement caused a crash in the initial
Android 14 emulator check and was corrected. No further emulator runs are used.

Each recording is capped at 30 minutes or 2 MiB. A current session and one previous
session are retained. Exports are immutable snapshots in a dedicated cache folder,
limited to three files. FileProvider grants read access only to the chosen export.
There is no external-storage permission, logcat capture or upload. Markers are user
observations, never device-state verification. CPU governor actions can also add
markers. JSON invalid/missing numeric data is null, not a fabricated zero.

The battery screen also reads en_power_path. Its kernel show handler initializes
its result true and ignores the charger driver's error return, so even a reported
one is diagnostic evidence only, not a guarantee of physical bypass. The raw
charging-disable request and signed battery current are logged alongside it.
