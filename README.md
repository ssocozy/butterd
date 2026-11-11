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

It discovers your kernel’s CPU/GPU controls, raises clock floors for demanding profiles, eases requests as heat builds, and releases them during idle or screen off. Android events drive updates, with periodic checks when events are unavailable. ROM/kernel limits remain in control. On HyperOS with its dynamic SurfaceFlinger CPU policy enabled, butter tunes CPU demand through foreground app thread clamps and leaves the shared PowerHAL frequency request and compositor scheduling to the ROM.

Your settings resume after reboot. Diagnostics show what butter requested and what the hardware actually allows. On supported MediaTek kernels, GPU floors use the APIBOOST limiter independently of PowerHAL’s FPSGO requests, preserving its ceiling and thermal priorities. Other kernels retain the existing GED/devfreq adapters. Stronger external GPU floor requests are preserved. Weaker replacements receive bounded GPU-specific retries; repeated conflicts pause GPU requests while independent CPU scheduling continues. It uses explicit rules, not machine learning.

## Install

1. Download **`butterd-VERSION.zip`** from [Releases](https://github.com/ssocozy/butterd/releases). Choose the module ZIP, not the sources archive.
2. Install through your root manager and reboot.
3. Open the module’s **WebUI**. Magisk requires a compatible WebUI host.

Settings and app profiles survive reboot. Stable updates appear through the module updater.

## Choose your preset

| Preset | Behavior |
| --- | --- |
| **Balanced** | Lets the ROM manage CPU/GPU scaling. |
| **Saver** | Lowers maximum clocks to reduce power use. |
| **Performance** | Raises minimum clocks, then eases them during heat or idle. A good starting point for games. |
| **Aggressive** | Starts with higher minimum clocks and gradually eases them as heat builds. Uses more power; higher clocks don’t guarantee smoother gameplay. |

For clock-floor controls, Performance starts near the middle of supported frequency steps; Aggressive starts around three-quarters. HyperOS CPU scheduling requests are described below. These are table positions, not percentages of GHz or performance. Both reduce requests in stages and release them at **42°C battery or 85°C SoC**, recovering after cooling. ROM/kernel limits always take priority.

In **Apps**, assign a preset, refresh rate, high touch sampling, or charging-pause threshold to individual apps. Their rules take priority over the overall preset while the app is open. The selected display refresh rate stays requested during cooling, charging and power saving; Butterd reapplies replaced display preferences and restores them when the app loses focus or the screen turns off. On compatible HyperOS owner-user sessions, it also sets PowerKeeper's per-app priority rate and restores the app's previous entry on release. PowerKeeper's list protocol rebuilds the priority map, merging other saved app entries at each write; concurrent ROM list updates can still race with that operation. On supported HyperOS builds, the active refresh preset also clears the thermal display-refresh vote and restores its previous value on release. CPU/GPU thermal protections remain active; ROM rewrites and hardware constraints can still affect actual display Hz. Performance app profiles include a five-second launch boost.

## What else is included?

- **Monitoring:** battery power, temperatures, display Hz, CPU/GPU clocks, battery drain and estimated deep sleep.
- **Running apps:** CPU/RAM readings and confirmed batch force-stop. Protected apps are hidden by default; nothing is killed automatically.
- **Hardware controls:** supported CPU governors, clock ranges, TCP algorithms and high touch sampling.
- **Bypass charging:** pauses battery charging while plugged in. The battery may still supply power. Manual use expires after 30 minutes; kernel protections stay active.
- **Phone microphone:** prefer the built-in mic with Bluetooth earbuds. App routing and ROM support can affect results.
- **CPU scheduling:** enabled by default with presets. Performance/Aggressive start with top-app minimum clamp requests of 50%/75%, reducing with heat and respecting existing maximums. Saver requests a ceiling at three-quarters of its captured maximum: per foreground app thread on HyperOS, or on the top-app group elsewhere. On HyperOS with the dynamic compositor CPU policy enabled, Performance/Aggressive apply 512/768 task clamp requests to foreground app threads instead of the shared top-app minimum. Existing task maximums and kernel group limits still apply; compositor scheduling stays ROM-managed. App CPU clamps and GPU requests work independently. Other ROMs also receive scoped compositor minimum clamps and CPU placement. Balanced, screen off, or disabling the setting restores owned values. ROM task profiles and controller definitions are preserved.
- **Vulkan UI rendering:** Vulkan rendering enabled by default for app interfaces, SystemUI and SurfaceFlinger composition. Change it in Settings and reboot; turn it off and reboot to use ROM defaults. The choice survives module updates. The WebUI reports the sampled SystemUI and composition backends separately from the selected preference; a ROM may retain OpenGL composition. Games retain their own rendering engines.
- **Recording assist:** optional higher clock requests during detected screen capture. Detection can take 15 seconds. Its SoC cutoff is 90°C; battery limits stay unchanged.

Engine notifications show the preset and cooling or error status. While charging, they also show battery temperature, `Charging` with watts, and percentage, updating silently about every 30 seconds. These readings disappear when charging stops.

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

Engine notifications use one silent entry for preset, bypass-charging and error updates. Cooling notices distinguish the requested preset from released boost requests. Routine screen-off/wake transitions do not announce the preset again.

## Troubleshooting

Open **Settings → Diagnostics → Export diagnostics** after reproducing an issue. Reports include app names; review them before sharing.

For optional logcat output, enable **Logcat events**, then run:

```sh
adb logcat -d -v threadtime -s butter:I > butter-logcat.txt
```

## Credits

[Encore](https://github.com/Rem01Gaming/encore) · [Duchamp Essential](https://github.com/Chandan977808/Duchamp-Essential) · [PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix)

[GPL-3.0](LICENSE) · [Third-party notices](THIRD_PARTY_NOTICES.txt)

Module updates refresh managed Pixel compatibility metadata while retaining the selected model, spoof switches and extra properties. Custom identities and newer security patch dates are preserved. The previous active profile is backed up as `compatibility/pif.previous.prop` when refreshed.
