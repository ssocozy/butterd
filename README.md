<div align="center">
  <img src="module/webroot/butter.png" width="128" alt="Butterd">
  <h1>Butterd</h1>
  <p>A performance module for POCO X6 Pro and Redmi K70E.</p>
  <p><a href="https://github.com/ssocozy/butterd/releases">Download</a> | <a href="https://github.com/ssocozy/butterd/issues">Issues</a></p>
</div>

Powered by **butter**, an optimization engine built for Duchamp’s Dimensity 8300 Ultra and Mali GPU.

## Controls

- Per-cluster CPU governors and CPU/GPU clock locking.
- Goodix 240/480 Hz touch modes and high sampling mode on supported firmware.
- Optional GPU cooling and charging thermal-vote overrides.
- Pixel 6a–9a compatibility profiles, fetched automatically during installation.

Configure controls in the module WebUI. Availability depends on your kernel and touch panel. Advanced thermal controls are opt-in and can increase heat and power use. Automatic game profiles are still in development.

## Installation

**Requirements:** Duchamp, Android 14+, arm64, and KernelSU or Magisk. Pixel compatibility requires Zygisk or a compatible KernelSU Zygisk provider.

1. Download the module ZIP from [Releases](https://github.com/ssocozy/butterd/releases).
2. Install it through your root manager and reboot.
3. Open the WebUI to configure Butterd. Magisk users need a compatible WebUI host.

Pixel 6a is selected by default, with build, Play Store build and scoped property spoofing enabled. Offline profiles are included. A Play Integrity verdict is not guaranteed.

## Credits

[Encore](https://github.com/Rem01Gaming/encore), [Duchamp Essential](https://github.com/Chandan977808/Duchamp-Essential), and [PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix).

[GPL-3.0](LICENSE) | [Third-party notices](THIRD_PARTY_NOTICES.txt)
