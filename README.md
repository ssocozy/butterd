# Butterd

A monochrome, read-only device monitor for POCO X6 Pro / Redmi K70E (Duchamp).
Java and XML, with white, black and system themes. Android 14 is the minimum;
Android 14–17 are the intended compatibility range. Android 17 tests skip with an
explicit notice if its emulator image is not published. CI emulator results, rather
than the presence of an SDK number, determine tested compatibility.

## Included in 0.1

- Battery percentage, temperature, voltage and signed net battery power.
- Session deep-sleep estimate from elapsed time minus uptime, without root.
- Android thermal severity; readable SoC temperature, CPU policy clocks, GED GPU
  frequency, ROM thermal profile and USB input voltage/current where available.
- Explicit, optional root access for a fixed list of read-only sensors. Denial,
  timeout or loss falls back to standard access. No root prompt on launch.
- A user-started, quiet live notification with a Stop action. Notification permission
  is requested only when enabling it; denied/blocked notifications leave it off.
- Rounded cards, scalable text, monochrome navigation icons and a translucent
  floating bottom bar with an opaque accessibility option. The bar uses a tinted
  surface, not expensive continuous backdrop capture or a claimed native iOS blur.

Battery power is **net power at the battery**, not wall/adapter wattage. Positive
current means entering the battery under Android's API contract; OEM sensor bugs
can still affect readings. USB voltage/current are separately labeled. Missing
or inaccessible measurements display “Unavailable”, never guessed values.
Deep sleep means device suspend, not screen-off time or Android Doze. The session
starts with the app process and resets when that process ends. Notification values
are the latest sample, not a promise of updates while the CPU is suspended.

## Safety boundary

This version has **no kernel-setting write path**, charging override, thermal
bypass, boot receiver, root module, partition access, SELinux changes, reboot
command, wake lock or network permission. Root only expands read access; it does
not unlock hidden dangerous controls. Butterd leaves ROM automation in charge.
Observation does not guarantee zero battery overhead: polling itself costs work.

## Get a build

Open [GitHub Actions](https://github.com/ssocozy/butterd/actions/workflows/android.yml),
select a successful run, and download `butterd-debug`. Extract and install the APK.
These are development APKs signed using the runner's debug key; signatures can
change between runs, requiring uninstall/reinstall. A stable release signing key
must be configured before distributing updateable production releases.

All compilation and execution of Android tests happens in **GitHub Actions**.
No local build is required or performed. The workflow runs JVM conversion tests,
Android lint, APK assembly, and emulator permission/navigation/theme/lifecycle
smoke tests. Device-test artifacts include screenshots and reports. Emulators
cannot verify Duchamp sensor availability, root managers, physical charging
accuracy, real suspend, thermal behavior or frame pacing on the phone.

See [design and safety decisions](docs/architecture.md),
[device source findings](docs/device-research.md), and
[manual acceptance checks](docs/device-validation.md).
