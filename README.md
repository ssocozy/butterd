<div align="center">
  <img src="module/webroot/butter.png" width="112" alt="Butterd">
  <h1>Butterd</h1>
  <p>Make your device optimized, one step at a time</p>
  <a href="https://github.com/ssocozy/butterd/releases"><img src="https://img.shields.io/github/downloads/ssocozy/butterd/total?style=flat-square&amp;color=181818&amp;label=downloads" alt="Downloads"></a>
  <p><a href="https://github.com/ssocozy/butterd/releases">Download</a> · <a href="https://github.com/ssocozy/butterd/issues">Report an issue</a></p>
</div>

A KernelSU / Magisk module for **POCO X6 Pro / Redmi K70E (Duchamp)**, powered by the **butter** Rust engine. Requires Android 14+ and arm64.

## Meet butter

**butter is the Rust engine behind Butterd.** It follows the foreground app, screen state, workload and temperatures to apply your presets and app rules.

It discovers your kernel’s CPU/GPU controls, raises clock floors for demanding profiles, eases requests as heat builds, and releases them during idle or screen off. Android events drive updates, with periodic checks when events are unavailable. ROM/kernel limits remain in control.

Your settings resume after reboot. Diagnostics show what butter requested and what the hardware actually allows. It uses explicit rules, not machine learning.

## Install

1. Download **`butterd-VERSION.zip`** from [Releases](https://github.com/ssocozy/butterd/releases). Choose the module ZIP, not the sources archive.
2. Install through your root manager and reboot.
3. Open the module’s **WebUI**. Magisk requires a compatible WebUI host.

Settings and app profiles survive reboot. Stable updates appear through the module updater.

## Choose your preset

| Preset | Behavior |
| --- | --- |
| **Balanced** | Lets the ROM manage CPU/GPU scaling. |
| **Battery** | Lowers maximum clocks to reduce power use. |
| **Performance** | Raises minimum clocks, then eases them during heat or idle. A good starting point for games. |
| **Aggressive** | Starts with higher minimum clocks and gradually eases them as heat builds. Uses more power; higher clocks don’t guarantee smoother gameplay. |

Performance starts near the middle of supported frequency steps; Aggressive starts around three-quarters. These are table positions, not percentages of GHz or performance. Both reduce requests in stages and release them at **42°C battery or 85°C SoC**, recovering after cooling. ROM/kernel limits always take priority.

In **Apps**, assign a preset, refresh rate, high touch sampling, or charging-pause threshold to individual apps. Their rules take priority over the overall preset while the app is open. Performance app profiles include a five-second launch boost.

## What else is included?

- **Monitoring:** battery power, temperatures, display Hz, CPU/GPU clocks, battery drain and estimated deep sleep.
- **Running apps:** CPU/RAM readings and confirmed batch force-stop. Protected apps are hidden by default; nothing is killed automatically.
- **Hardware controls:** supported CPU governors, clock ranges, TCP algorithms and high touch sampling.
- **Bypass charging:** pauses battery charging while plugged in. The battery may still supply power. Manual use expires after 30 minutes; kernel protections stay active.
- **Phone microphone:** prefer the built-in mic with Bluetooth earbuds. App routing and ROM support can affect results.
- **Recording assist:** optional higher clock requests during detected screen capture. Detection can take 15 seconds. Its SoC cutoff is 90°C; battery limits stay unchanged.

CPU/GPU requests, effective limits and current clocks are separate readings. Features depend on what your ROM and kernel expose. Battery history comes from Android; it isn’t an instantaneous per-app power measurement.

## Pixel compatibility

Play Integrity Fix is intentionally included with **build, Play Store build and scoped property spoofing** enabled. Installation selects a Pixel 6a–9a profile automatically, with offline profiles available. Updates preserve your existing profile.

Change the model in the WebUI. Applying it restarts the affected Google processes; Play Store may close briefly. Requires Zygisk or a compatible KernelSU Zygisk provider.

This can complement a compatible [Fenrir](https://github.com/R0rt1z2/fenrir) setup, but Butterd does not install Fenrir, modify your bootloader, or supply keyboxes. **No Play Integrity verdict is guaranteed.**

## Before you use it

> [!WARNING]
> Root tuning can increase heat, battery use and instability. Avoid overlapping tuning modules. Release Butterd’s controls before disabling it, then reboot. Keep a working module ZIP and know your root manager’s recovery procedure.

> [!IMPORTANT]
> Releases pass automated checks before publication; device testing is ongoing. Butterd cannot guarantee better FPS or battery life on every ROM. Use at your own risk; no warranty is provided. See [LICENSE](LICENSE).

Engine notifications use one silent entry for preset, bypass-charging and error updates. Routine screen-off/wake transitions do not announce the preset again.

## Troubleshooting

Open **Settings → Diagnostics → Export diagnostics** after reproducing an issue. Reports include app names; review them before sharing.

For optional logcat output, enable **Logcat events**, then run:

```sh
adb logcat -d -v threadtime -s butter:I > butter-logcat.txt
```

## Credits

[Encore](https://github.com/Rem01Gaming/encore) · [Duchamp Essential](https://github.com/Chandan977808/Duchamp-Essential) · [PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix)

[GPL-3.0](LICENSE) · [Third-party notices](THIRD_PARTY_NOTICES.txt)
