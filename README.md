<div align="center">
  <img src="module/webroot/butter.png" width="128" alt="Butterd">
  <h1>Butterd</h1>
  <a href="https://github.com/ssocozy/butterd/releases"><img src="https://img.shields.io/github/downloads/ssocozy/butterd/total?style=flat-square&amp;color=181818&amp;label=downloads" alt="Total release downloads"></a>
  <p>Make your device optimized, one step at a time</p>
  <p><a href="https://github.com/ssocozy/butterd/releases">Download</a> | <a href="https://github.com/ssocozy/butterd/issues">Issues</a></p>
</div>

Powered by **butter**, a Rust optimization engine built for Duchamp’s Dimensity 8300 Ultra and Mali GPU.

## Presets

| Preset | What it changes |
| --- | --- |
| **Balanced** | Releases the CPU/GPU limits applied by the preset and returns those limits to ROM control. Separate manual governor and frequency settings stay separate. |
| **Performance** | Raises minimum CPU/GPU clocks to about one-third through the available frequency steps, keeping the existing maximum limits. Clocks can still scale with demand. |
| **Battery** | Keeps the original minimum clocks and lowers maximum clocks to about three-quarters through the available frequency steps. This can reduce responsiveness under heavier loads. |

Frequency steps come from the running kernel and current limits. These positions are not percentages of GHz, performance or battery savings. GPU tuning requires a readable frequency table. Presets do not change CPU governors or disable thermal protections.

With automatic profiles enabled, the foreground app’s rule takes precedence over the overall preset. Entering an app assigned **Performance** requests a **3-second launch boost**, subject to the same heat and battery checks. Other apps use the overall preset. Scrolling and video playback are not detected separately yet.

Butter releases preset limits when the screen turns off or the battery reaches 42°C or sampled SoC temperature reaches 85°C. After cooling starts, it waits for battery temperature below 39°C and sampled SoC below 75°C before resuming. Android Battery Saver or a battery level of 20% or less selects **Battery** when heat checks allow tuning. Missing required readings prevent performance requests. Performance floors can also be released after three low-activity samples, but only when both CPU and GPU activity are readable.

Per-app refresh rates are independent of CPU/GPU presets. Leaving an app releases its display request and restores its previous ROM refresh rule where a compatible service is available; the next app follows its own rule or the ROM defaults. A 60 Hz request remains eligible while CPU performance is paused for heat.

## Controls

- Balanced, Performance and Battery CPU/GPU presets, with one governor selector for all CPU clusters and advanced per-cluster controls.
- Opt-in foreground app profiles, temporary launch boosts and per-app display preferences.
- Battery, temperature, workload and screen-state monitoring, with explainable profile decisions.
- TCP congestion-control selection from your kernel’s available algorithms.
- Goodix 240/480 Hz touch modes and high sampling mode on supported firmware.
- Optional, time-limited clock, GPU cooling and charging thermal-vote overrides.
- Observable kernel battery bypass where supported. The stock-kernel charging pause coordinates a zero charge-current request with primary-charger pause; it does not establish physical battery isolation. Battery current remains visible for phone testing.
- HTSR handover stops only XiaomiParts’ touch service while Butterd owns the request and restores its previous running state when released. It does not disable the package or persistently disable the component; an unexpected service restart is a conflict.
- Phone-microphone capture preferences with Bluetooth earbuds on ROMs exposing the privileged routing API.
- Local diagnostic reports, optional logcat events and app frame-duration samples.
- Pixel 6a–9a compatibility profiles, fetched automatically during installation.

Configure controls in the module WebUI. Optional engine notifications show startup progress and applied preset changes using one Android system notification; transient boosts and repeated samples do not create alerts. Delivery depends on the ROM’s notification settings. Overview holds the overall presets; Apps lists installed launchable apps with searchable profiles; Settings holds hardware controls and diagnostics. Availability depends on your kernel and touch panel. Advanced thermal controls are opt-in and can increase heat and power use. Monitoring starts at boot. Applied presets and manual controls are saved and rechecked before resuming after reboot. Unsupported controls or conflicts remain visible instead of being forced. Temporary clock and thermal overrides keep their original expiration time; a reboot does not renew them. Saving an app profile enables automatic tuning; the Apps switch can pause it. Initial frequency-table presets still need device benchmarks.

## How butter works

butter is a device-aware Rust control engine for Duchamp. Its supervisor wakes on Android app-lifecycle, screen and battery events, then reads authoritative state before applying a rule. A bounded polling fallback covers missing ROM events and thermal checks. It follows the foreground app, screen state, battery saver, battery temperature and sampled CPU activity. App rules choose a preset; entering a configured Performance app automatically triggers a short launch boost, which expires without a permanent clock lock. When both CPU and GPU activity can be measured as idle, performance floors are released; heat takes priority over boosts.

The engine discovers CPU policies and Mali frequency tables at runtime. On kernels exposing the verified six-value MediaTek PowerHAL ABI, CPU presets use that QoS request with bounded retries; other layouts use cpufreq. Thermal policy stays unchanged, and persistent conflicts still pause tuning. Performance raises moderate frequency floors; Battery lowers ceilings; Balanced returns profile limits to the ROM. It does not assume every kernel exposes the same nodes. Display requests combine advertised preferred modes, Android refresh bounds and recognised existing ROM refresh settings. Compatible ROM refresh-manager services are detected by their live interface and method signatures; butter temporarily updates the active app’s rule through the ROM API instead of replacing the full app list. ROMs without that service use the Android display path. Original values, including an unlimited peak, are restored when the app leaves; newer ROM changes are preserved.

These are explicit policies, not machine learning. The frequency choices are conservative starting points, not proven efficiency sweet spots. Diagnostics provide readings and recent app-rendered frame durations so changes can be compared on the same workload. No FPS or battery-life improvement is claimed without device measurements.

> [!IMPORTANT]
> Automated source checks, unit tests and release-build checks run before publication. On-device validation is ongoing; passing CI does not establish stability on every ROM or kernel. Keep a known-working module ZIP and know how to disable modules through your root manager's recovery procedure.

> [!WARNING]
> Root controls can increase heat, power consumption and instability. Advanced overrides are off by default and expire automatically, but they cannot make unsupported hardware behavior safe. Avoid overlapping performance controllers. Use Butterd at your own risk; the project provides no warranty and accepts no responsibility for damage, data loss or boot failures caused by misuse or incompatible modifications. See [LICENSE](LICENSE).

## Installation

**Requirements:** Duchamp, Android 14+, arm64, and KernelSU or Magisk. Pixel compatibility requires Zygisk or a compatible KernelSU Zygisk provider.

1. Download `butterd-VERSION.zip` (the `-sources.zip` archive is for source code) from [Releases](https://github.com/ssocozy/butterd/releases).
2. Install it through your root manager and reboot.
3. Open the WebUI to configure Butterd. Magisk users need a compatible WebUI host.

Fresh installs randomly select a Pixel profile from 6a through 9a, with build, Play Store build and scoped property spoofing enabled. Change the model in the WebUI; applying it restarts only the Google integrity and Play Store target processes so no phone reboot is needed for profile changes. Updates preserve your complete existing PIF profile and settings. Reapplying a model in the WebUI explicitly replaces the identity with that bundled profile. Offline profiles include the model’s initial API and a matching published Pixel security patch. These identity properties are scoped to the target process; the Android SDK version is unchanged. A Play Integrity verdict is not guaranteed.

## Phone microphone with Bluetooth earbuds

In **Settings → Miscellaneous**, connect Bluetooth earbuds and select **Prefer phone microphone**. Butter requests the built-in microphone for ordinary recording, camera recording and voice communication capture presets. This is a shared preference across apps using those presets; explicit app routing, phone calls and vendor audio policies can take precedence. Bluetooth output is not selected or rewritten by Butter.

The preference is saved, resumes when earbuds are connected, and restores the previous capture preferences on disconnect or when disabled. It never records audio. The WebUI shows active input devices when Android reports them; saved preference readback alone is not proof that a call or game uses the phone mic. Unsupported APIs and permission errors are shown. Device testing is required separately for recording, games and calls.

## Why Play Integrity Fix is included

PIF is intentionally bundled alongside the performance engine for compatibility on rooted Duchamp setups. Its build, Play Store build and scoped property spoofing can complement an existing, compatible [Fenrir](https://github.com/R0rt1z2/fenrir) setup when seeking STRONG integrity.

Fenrir operates in the device bootchain; it is not a PIF option or a standard ROM toggle. Butterd does not install it or patch your bootloader. Without an appropriate attestation setup, these three spoofing options alone do not provide STRONG integrity. A separate attestation solution, such as one using suitable RKP-backed keyboxes, may be needed. Butterd neither includes nor fetches keyboxes.

STRONG integrity is not guaranteed by PIF, Fenrir or a keybox. The result depends on the device, ROM, attestation validity, security-update requirements and Google's checks. See [Google's verdict documentation](https://developer.android.com/google/play/integrity/verdicts).

## Diagnostics

Enable logcat output in the WebUI, reproduce the issue, then export the diagnostics report. The Overview page distinguishes a stopped supervisor from sensors that firmware does not expose. Reports remain available without acquiring the control-write lock and include retained startup errors. Engine messages use the `butter` tag. Reports include app package names and configuration; review them before sharing. Frame samples describe recent app-rendered durations, not game FPS.

## Credits

[Encore](https://github.com/Rem01Gaming/encore), [Duchamp Essential](https://github.com/Chandan977808/Duchamp-Essential), and [PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix).

[GPL-3.0](LICENSE) | [Third-party notices](THIRD_PARTY_NOTICES.txt)
