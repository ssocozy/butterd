<div align="center">
  <img src="module/webroot/butter.png" width="128" alt="Butterd">
  <h1>Butterd</h1>
  <p>Performance controls for your Duchamp.</p>
  <p><a href="https://github.com/ssocozy/butterd/releases">Download</a> | <a href="https://github.com/ssocozy/butterd/issues">Report an issue</a> | <a href="LICENSE">GPL-3.0</a></p>
</div>

Butterd is a KernelSU and Magisk module for the **POCO X6 Pro / Redmi K70E**. Its optimization engine, **butter**, controls the hardware interfaces exposed by your kernel, with saved originals and verified changes.

### Take control

| | What you can change |
|---|---|
| **CPU** | Select a governor per cluster or request its highest supported clock within the current limit. |
| **GPU** | Request a Mali clock lock when the kernel exposes a readable frequency table. |
| **Touch** | Request native 240/480 Hz through Goodix, or high sampling mode on supported firmware. |
| **Advanced** | Opt into the Mali cooling override or the charging thermal-vote override, separately. |
| **Pixel profiles** | Switch between eleven Pixel identities, from Pixel 6a through Pixel 9a. |

Advanced controls start off. Each requires confirmation, saves the original values and provides **Restore**. Requests are session-based; the ROM can replace them. Automatic game profiles and the broader profile engine are still being developed.

### Install

1. Use **Duchamp, Android 14+, arm64**, with KernelSU or Magisk.
2. Enable Zygisk in Magisk, or install a compatible Zygisk provider for KernelSU, for Pixel compatibility.
3. Download the module ZIP from [Releases](https://github.com/ssocozy/butterd/releases), install it in your root manager and reboot.
4. Open the module WebUI. Magisk users need a compatible WebUI host.

Installation fetches fresh Pixel metadata automatically and keeps bundled profiles for offline installation. **Pixel 6a** is selected initially. Build, Play Store build and scoped property spoofing are enabled by default; changing models needs no manual fetch. Restart the affected apps or reboot after switching.

### Know what is being requested

- Native **480 Hz** may appear as roughly **500 Hz** in millisecond-based touch testers. A sampling-mode flag is not a measured report rate.
- The charging override removes **one thermal vote**, not every charger limit. The Mali override requests cooling state zero; other thermal protections can still apply.
- Clock locks and thermal overrides can increase heat and power consumption. They require an explicit opt-in and are never replayed at boot. Unexposed controls stay disabled.
- Pixel spoofing needs a working Zygisk environment and does not guarantee a Play Integrity verdict. Provider/signature hooks and SDK spoofing are excluded.

Avoid running multiple tuning controllers or PIF implementations together. Restore changes before disabling/removing Butterd. If the ROM changes a setting, restoration refuses to overwrite it blindly; reboot clears the session.

### Terminal

```sh
su
/data/adb/modules/butterd/bin/butterctl status
/data/adb/modules/butterd/bin/butterctl governor policy4 schedutil
/data/adb/modules/butterd/bin/butterctl restore policy4
/data/adb/modules/butterd/bin/butter status
/data/adb/modules/butterd/bin/butter enable touch 480
/data/adb/modules/butterd/bin/butter restore touch
```

### Development

Builds run through the manual [Module workflow](https://github.com/ssocozy/butterd/actions/workflows/module.yml). Each ZIP includes the native compatibility source. Development builds require testing on the phone; compilation alone does not establish hardware compatibility or performance gains.

Built by [ssocozy](https://github.com/ssocozy). Compatibility code derives from [PlayIntegrityFix](https://github.com/KOWX712/PlayIntegrityFix); [Encore](https://github.com/Rem01Gaming/encore) and [Duchamp Essential](https://github.com/Chandan977808/Duchamp-Essential) inform the hardware review. See [third-party notices](THIRD_PARTY_NOTICES.txt) for licenses and retained components.
