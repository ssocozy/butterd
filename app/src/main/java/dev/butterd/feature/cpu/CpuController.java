package dev.butterd.feature.cpu;

import android.content.Context;
import android.content.SharedPreferences;
import dev.butterd.core.root.RootExecutor;

/** Manual, per-policy governor changes only. Never writes frequency, online, thermal or boot nodes. */
public final class CpuController {
    private final SharedPreferences journal;
    public CpuController(Context context) { journal = context.getSharedPreferences("cpu_session", Context.MODE_PRIVATE); }
    public boolean hasSaved(String policy) { return journal.contains(policy + ".original"); }
    public void apply(String policy, String target, boolean restore) throws Exception {
        if (!CpuPolicy.validId(policy) || (!restore && !CpuPolicy.selectable(target))) throw new IllegalArgumentException("Unsupported governor");
        String inventory = inventory(policy);
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
        if (!"VERIFIED".equals(RootExecutor.run(script, 10, 1024)))
            throw new IllegalStateException("Governor readback failed; saved original retained");
        if (restore) clear(policy);
    }
    private static String inventory(String policy) throws Exception {
        String path = "/sys/devices/system/cpu/cpufreq/" + policy;
        String script = "printf 'BOOT|%s\\n' \"$(head -c 40 /proc/sys/kernel/random/boot_id)\"; "
                + "printf 'C|%s|%s|%s|%s|%s|%s|1\\n' '" + policy + "' "
                + "\"$(head -c 32 " + path + "/scaling_governor)\" "
                + "\"$(head -c 160 " + path + "/scaling_available_governors)\" "
                + "\"$(head -c 64 " + path + "/related_cpus)\" "
                + "\"$(head -c 24 " + path + "/scaling_min_freq)\" "
                + "\"$(head -c 24 " + path + "/scaling_max_freq)\"";
        return RootExecutor.run(script, 20, 4096);
    }
    private void clear(String policy) {
        journal.edit().remove(policy + ".original").remove(policy + ".expected").remove(policy + ".boot").commit();
    }
}
