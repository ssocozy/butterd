package dev.butterd;

import android.content.Context;
import android.content.SharedPreferences;
import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;

/** Manual, per-policy governor changes only. Never writes frequency, online, thermal or boot nodes. */
final class CpuController {
    private final SharedPreferences journal;
    CpuController(Context context) { journal = context.getSharedPreferences("cpu_session", Context.MODE_PRIVATE); }
    boolean hasSaved(String policy) { return journal.contains(policy + ".original"); }
    void apply(String policy, String target, boolean restore) throws Exception {
        if (!CpuPolicy.validId(policy) || (!restore && !CpuPolicy.selectable(target))) throw new IllegalArgumentException("Unsupported governor");
        String inventory = RootReader.read();
        String boot = ""; CpuPolicy found = null;
        for (String line : inventory.split("\n")) {
            String[] p = line.split("\\|", -1);
            if (p.length == 2 && p[0].equals("BOOT")) boot = p[1];
            if (p.length == 8 && p[0].equals("C") && p[1].equals(policy))
                found = new CpuPolicy(p[1], p[2], p[3], p[4], p[5], p[6]);
        }
        if (!boot.matches("[a-f0-9\\-]{36}") || found == null) throw new IllegalStateException("CPU control could not be verified");
        String original = journal.getString(policy + ".original", null);
        String expected = journal.getString(policy + ".expected", null);
        if (original != null && !boot.equals(journal.getString(policy + ".boot", ""))) {
            clear(policy); original = null; expected = null;
            if (restore) return; // The kernel already reset its state on reboot.
        }
        if (restore && original == null) throw new IllegalStateException("No saved governor for this boot");
        if (original != null && found.governor.equals(original) && restore) { clear(policy); return; }
        if (original != null && !found.governor.equals(expected)) throw new IllegalStateException("Another controller changed this governor. Nothing was overwritten.");
        if (restore) target = original;
        if (!CpuPolicy.validGovernor(target) || !found.governors.contains(target)) throw new IllegalStateException("Governor is no longer available");
        if (found.governor.equals(target)) return;
        if (!journal.edit().putString(policy + ".original", original == null ? found.governor : original)
                .putString(policy + ".expected", target).putString(policy + ".boot", boot).commit())
            throw new IllegalStateException("Could not save the original governor; no change made");
        String path = "/sys/devices/system/cpu/cpufreq/" + policy + "/scaling_governor";
        // Both interpolated values have strict ASCII validation above. Check ownership again
        // immediately before writing; this reduces races but is not a kernel-level lock.
        String script = "[ \"$(id -u)\" = 0 ] || exit 1; "
                + "[ \"$(cat " + path + ")\" = '" + found.governor + "' ] || exit 2; "
                + "printf '%s' '" + target + "' > " + path + " || exit 3; "
                + "[ \"$(cat " + path + ")\" = '" + target + "' ] || exit 4; printf 'VERIFIED\\n'";
        Process process = new ProcessBuilder("su", "-c", script).redirectErrorStream(true).start();
        try {
            if (!process.waitFor(10, TimeUnit.SECONDS) || process.exitValue() != 0)
                throw new IllegalStateException("Governor change was not verified. Use Restore to check recovery.");
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
                if (!"VERIFIED".equals(reader.readLine())) throw new IllegalStateException("Governor readback failed; saved original retained");
            }
        } finally { process.destroy(); }
        if (restore) clear(policy);
    }
    private void clear(String policy) {
        journal.edit().remove(policy + ".original").remove(policy + ".expected").remove(policy + ".boot").commit();
    }
}
