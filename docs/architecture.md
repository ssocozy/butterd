# Architecture and safety decisions

## Process and permissions

One Application-owned repository samples on a background HandlerThread. UI and
notification observers share it. Sampling stops when there are no observers;
there is no independent polling loop per screen. Android APIs provide standard
battery telemetry and thermal severity. Readable sysfs supplies optional sensors.
Root is requested only from the user's access button and is not persisted across
process restarts. A failed root read clears privileged data and falls back to
standard telemetry. su receives a fixed command, never paths or commands from an
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
leave nothing to restore because no device settings are written.

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
No unsupported safety guarantees such as “cannot brick” are made; the first
version removes kernel writes from the application entirely.
