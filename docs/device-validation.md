# Physical-device acceptance checks

CI smoke tests verify Android UI and permission/service flows, not real hardware.
Before recommending the development APK on Duchamp:

1. Launch without root. Confirm no root or notification prompt appears and no
   service runs until requested. Missing sensors must stay unavailable.
2. Compare battery voltage, temperature and signed current with trustworthy gauge
   readings. Cross-check USB values separately using a meter when possible;
   battery power must never be presented as adapter power.
3. Deny root and dismiss a root prompt. Standard telemetry must continue, with a
   useful error. Grant on explicit retry. Revoke in the root manager and confirm
   fallback clears privileged readings. Stop using root in-app.
4. Enable notifications, deny permission, then grant through settings. Verify the
   switch matches the real service, blocked channels are respected, Stop works,
   and force stop/reboot does not silently restart monitoring.
5. Turn the screen off long enough for device suspend. Check that deep sleep rises
   only when the CPU actually suspends; compare with system diagnostics. The app
   must not keep the CPU awake to update its notification.
6. Check black, white and system themes, gesture/three-button navigation, landscape,
   TalkBack and 200% font size. All pages must scroll past the floating navigation.
7. Inspect frame timing on the physical phone while scrolling with monitoring and
   root enabled. No smoothness or thermal/power improvements are claimed yet.
8. Confirm ROM thermal-profile changes appear in Butterd without Butterd writing
   back or fighting XiaomiParts. Confirm no kernel settings change on exit.

For 0.2, also check:

9. Open each detail screen from Overview, navigate back, rotate, and test large
   fonts. CPU changes must be disabled without root. GPU frequency writes and
   charging overrides must not be offered.
10. If testing a CPU governor, record the original, choose one exposed allowlisted
    governor, verify readback and restore it. Repeat after reopening the app. Do
    not assume ROM concurrency is solved: unexpected external changes must produce
    a conflict instead of forced restoration. No governor is applied at boot.
11. Start/stop a recording, add markers and share it. Inspect JSONL for unavailable
    null values, charging sample ages and expected sensor values. Confirm Stop in
    the notification ends recording and revoking notification permission stops it.
    Check storage/time limits and ensure sharing requires an explicit user action.
12. To investigate the existing ROM bypass, record normal charging, the lower-current
    selection, bypass on/off, screen changes and reconnect. Use markers and compare
    battery flow with adapter measurements. Butterd does not enact the sequence.
