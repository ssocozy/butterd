# Butterd

KernelSU / Magisk module for Duchamp (POCO X6 Pro / Redmi K70E), Android 14 or newer, arm64.

The Android application has been archived separately. This repository now contains the module.

## Included

- Hardware capability report: CPU policies, governors and frequency tables; GED GPU reading; TCP algorithms; HTSR flag; supported bypass capability flag.
- Manual, verified per-cluster CPU governor changes with saved originals and conflict detection. No tuning is automatically applied at boot.
- Reduced, process-scoped PlayIntegrityFix compatibility: `spoofBuild`, `spoofVendingBuild` and `spoofProps`, enabled by default.

There are no provider/signature hooks, SDK overrides, keyboxes, Google data clearing or global property overrides. Compatibility needs Magisk Zygisk or a compatible Zygisk provider under KernelSU. KernelSU alone does not supply Zygisk. No integrity verdict is guaranteed; the bundled upstream identity can expire or be rejected.

## Usage

Install the module ZIP from a GitHub release through your root manager, then reboot. Do not install alongside active performance controllers or another PIF implementation. Installation rejects several known conflicting module IDs; it cannot identify every renamed module or ROM controller.

From a root shell:

```sh
/data/adb/modules/butterd/bin/butterctl status
/data/adb/modules/butterd/bin/butterctl governor policy4 schedutil
/data/adb/modules/butterd/bin/butterctl restore policy4
```

Only governors reported by that policy and supported by Butterd can be selected. Values are verified after writing. If another controller changes the value, Butterd refuses to overwrite it. Restore before disabling/removing the module; CPU changes remain for the current boot otherwise. Reboot returns control to the ROM. No CPU/GPU frequency presets or per-app automation are implemented yet.

Installation automatically refreshes Pixel 6a through Pixel 9a metadata using certificate-validated Google HTTPS endpoints. All eleven models have bundled offline profiles; no manual fetch is needed. Pixel 6a is the initial selection. The WebUI compatibility section only changes the selected model; the three spoofing options are enabled by default. Updates retain the selected model.

Compatibility settings live in `compatibility/pif.prop` inside the installed module. All three can be disabled individually; restart the affected apps or reboot for changes to take effect. Disabling/removing the module requires reboot to remove already loaded process-local changes. Kernel thermal limits, voltage, charging controls, partitions and system files are untouched.

## Build

The manual GitHub Actions workflow tests the configuration parser, validates module structure, cross-compiles the arm64 Zygisk library and publishes an installable ZIP. No local compilation or emulator testing. Phone validation is required before considering this stable. Automatic foreground-game profiles and the broader performance engine are still under development; the current module exposes manual governor controls and diagnostics.

GPL-3.0; see LICENSE and THIRD_PARTY_NOTICES.txt. Corresponding compatibility source is included in each module ZIP.
