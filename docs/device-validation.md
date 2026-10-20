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
