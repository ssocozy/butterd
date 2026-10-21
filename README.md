<div align="center">
  <img src="module/webroot/butter.png" width="128" alt="Butterd">
  <h1>Butterd</h1>
  <p>A performance module for POCO X6 Pro and Redmi K70E.</p>
  <p><a href="https://github.com/ssocozy/butterd/releases">Download</a> | <a href="https://github.com/ssocozy/butterd/issues">Issues</a></p>
</div>

Powered by **butter**, a Rust optimization engine built for Duchamp’s Dimensity 8300 Ultra and Mali GPU.

## Controls

- Balanced, Performance and Battery CPU/GPU presets, with per-cluster governor and frequency controls.
- Opt-in foreground app profiles, temporary launch boosts and per-app display preferences.
- Battery, temperature, workload and screen-state monitoring, with explainable profile decisions.
- TCP congestion-control selection from your kernel’s available algorithms.
- Goodix 240/480 Hz touch modes and high sampling mode on supported firmware.
- Optional, time-limited clock, GPU cooling and charging thermal-vote overrides.
- Observable kernel battery bypass where supported; a separately labelled experimental primary-charger pause.
- Local diagnostic reports, optional logcat events and app frame-duration samples.
- Pixel 6a–9a compatibility profiles, fetched automatically during installation.

Configure controls in the module WebUI. Overview holds the overall presets; Apps lists installed launchable apps with searchable profiles; Settings holds hardware controls and diagnostics. Availability depends on your kernel and touch panel. Advanced thermal controls are opt-in and can increase heat and power use. Read-only monitoring starts at boot. Automatic tuning starts only when enabled; choose installed apps to add profile rules. Initial frequency-table presets still need device benchmarks.

## How butter works

butter is a device-aware Rust control engine for Duchamp. Its supervisor follows the foreground app, screen state, battery saver, battery temperature and sampled CPU activity. App rules choose a preset; short boost leases overlap independently and expire without a permanent clock lock. When both CPU and GPU activity can be measured as idle, performance floors are released; heat takes priority over boosts.

The engine discovers CPU policies and Mali frequency tables at runtime. Performance raises moderate frequency floors; Battery lowers ceilings; Balanced returns profile limits to the ROM. It does not assume every kernel exposes the same nodes. Display requests use Android preferences, accept only advertised internal-display modes and yield when another controller changes those preferences.

These are explicit policies, not machine learning. The frequency choices are conservative starting points, not proven efficiency sweet spots. Diagnostics provide readings and recent app-rendered frame durations so changes can be compared on the same workload. No FPS or battery-life improvement is claimed without device measurements.

> [!IMPORTANT]
> Automated source checks, unit tests and release-build checks run before publication. This version is awaiting physical-device validation; passing CI does not establish stability on every ROM or kernel. Keep a known-working module ZIP and know how to disable modules through your root manager's recovery procedure.

> [!WARNING]
> Root controls can increase heat, power consumption and instability. Advanced overrides are off by default and expire automatically, but they cannot make unsupported hardware behavior safe. Avoid overlapping performance controllers. Use Butterd at your own risk; the project provides no warranty and accepts no responsibility for damage, data loss or boot failures caused by misuse or incompatible modifications. See [LICENSE](LICENSE).

## Installation

**Requirements:** Duchamp, Android 14+, arm64, and KernelSU or Magisk. Pixel compatibility requires Zygisk or a compatible KernelSU Zygisk provider.

1. Download `butterd-VERSION.zip` (the `-sources.zip` archive is for source code) from [Releases](https://github.com/ssocozy/butterd/releases).
2. Install it through your root manager and reboot.
3. Open the WebUI to configure Butterd. Magisk users need a compatible WebUI host.

Fresh installs randomly select a Pixel profile from 6a through 9a, with build, Play Store build and scoped property spoofing enabled. Change the model in the WebUI; updates keep your selection. Offline profiles include the model’s initial API and a matching published Pixel security patch. These identity properties are scoped to the target process; the Android SDK version is unchanged. A Play Integrity verdict is not guaranteed.

## Why Play Integrity Fix is included

PIF is intentionally bundled alongside the performance engine for compatibility on rooted Duchamp setups. Its build, Play Store build and scoped property spoofing can complement an existing, compatible [Fenrir](https://github.com/R0rt1z2/fenrir) setup when seeking STRONG integrity.

Fenrir operates in the device bootchain; it is not a PIF option or a standard ROM toggle. Butterd does not install it or patch your bootloader. Without an appropriate attestation setup, these three spoofing options alone do not provide STRONG integrity. A separate attestation solution, such as one using suitable RKP-backed keyboxes, may be needed. Butterd neither includes nor fetches keyboxes.

STRONG integrity is not guaranteed by PIF, Fenrir or a keybox. The result depends on the device, ROM, attestation validity, security-update requirements and Google's checks. See [Google's verdict documentation](https://developer.android.com/google/play/integrity/verdicts).

## Diagnostics

Enable logcat output in the WebUI, reproduce the issue, then export the diagnostics report. The Overview page distinguishes a stopped supervisor from sensors that firmware does not expose. Reports remain available without acquiring the control-write lock and include retained startup errors. Engine messages use the `butter` tag. Reports include app package names and configuration; review them before sharing. Frame samples describe recent app-rendered durations, not game FPS.

## Credits

[Encore](https://github.com/Rem01Gaming/encore), [Duchamp Essential](https://github.com/Chandan977808/Duchamp-Essential), and [PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix).

[GPL-3.0](LICENSE) | [Third-party notices](THIRD_PARTY_NOTICES.txt)
