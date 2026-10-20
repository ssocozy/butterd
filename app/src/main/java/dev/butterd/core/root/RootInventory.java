package dev.butterd.core.root;

import dev.butterd.feature.kernel.KernelSensors;

/** No arbitrary command API. Only this fixed, bounded sensor inventory can run as root. */
public final class RootInventory {
    private static final String SCRIPT =
            "[ \"$(id -u)\" = 0 ] || exit 1; printf 'ROOT|0\\n'; " +
            "printf 'BOOT|%s\\n' \"$(head -c 40 /proc/sys/kernel/random/boot_id)\"; " +
            "n=0; for z in /sys/class/thermal/thermal_zone*; do " +
            "[ -r \"$z/type\" ] && [ -r \"$z/temp\" ] || continue; " +
            "printf 'T|%s|%s\\n' \"$(head -c 64 \"$z/type\")\" \"$(head -c 24 \"$z/temp\")\"; " +
            "n=$((n+1)); [ \"$n\" -ge 64 ] && break; done; " +
            "n=0; for p in /sys/devices/system/cpu/cpufreq/policy*; do " +
            "[ -r \"$p/scaling_cur_freq\" ] || continue; " +
            "printf 'F|%s|%s\\n' \"${p##*/}\" \"$(head -c 24 \"$p/scaling_cur_freq\")\"; " +
            "printf 'C|%s|%s|%s|%s|%s|%s|1\\n' \"${p##*/}\" \"$(head -c 32 \"$p/scaling_governor\")\" \"$(head -c 160 \"$p/scaling_available_governors\")\" \"$(head -c 64 \"$p/related_cpus\")\" \"$(head -c 24 \"$p/scaling_min_freq\")\" \"$(head -c 24 \"$p/scaling_max_freq\")\"; " +
            "n=$((n+1)); [ \"$n\" -ge 16 ] && break; done; " +
            "printf 'K|%s\\n' \"$(head -c 24 /sys/class/power_supply/battery/constant_charge_current 2>/dev/null)\"; " +
            "printf 'B|%s\\n' \"$(head -c 32 /proc/mtk_battery_cmd/current_cmd 2>/dev/null)\"; " +
            "printf 'H|%s\\n' \"$(head -c 16 /proc/mtk_battery_cmd/en_power_path 2>/dev/null)\"; " +
            "printf 'O|%s\\n' \"$(head -c 64 /sys/devices/system/cpu/online 2>/dev/null)\"; " +
            "printf 'P|%s\\n' \"$(head -c 24 /sys/class/thermal/thermal_message/sconfig 2>/dev/null)\"; " +
            "printf 'V|%s\\n' \"$(head -c 24 /sys/class/power_supply/usb/voltage_now 2>/dev/null)\"; " +
            "printf 'I|%s\\n' \"$(head -c 24 /sys/class/power_supply/usb/current_now 2>/dev/null)\"; " +
            "printf 'G|%s\\n' \"$(head -c 80 /sys/kernel/ged/hal/current_freqency 2>/dev/null)\"";

    private static String extras() {
        StringBuilder script = new StringBuilder();
        for (java.util.Map.Entry<String, String> sensor : KernelSensors.PATHS.entrySet())
            script.append("; printf 'D|").append(sensor.getKey()).append("|%s\\n' \"$(head -c 160 ")
                    .append(sensor.getValue()).append(" 2>/dev/null)\"");
        script.append("; n=0; for p in /sys/devices/system/cpu/cpufreq/policy*; do printf 'A|%s|%s\\n' \"${p##*/}\" \"$(head -c 512 \"$p/scaling_available_frequencies\" 2>/dev/null)\"; n=$((n+1)); [ \"$n\" -ge 16 ] && break; done");
        return script.append("; exit 0").toString();
    }
    public static void verifyAccess() throws Exception {
        String result = RootExecutor.run("printf 'BUTTERD_UID|%s\\n' \"$(id -u)\"", 25, 2048);
        if (!java.util.Arrays.asList(result.split("\\n")).contains("BUTTERD_UID|0"))
            throw new SecurityException("The root shell did not verify UID 0");
    }
    static String command() { return SCRIPT + extras(); }
    public static String read() throws Exception {
        String inventory = RootExecutor.run(command(), 20, 32768);
        if (!inventory.startsWith("ROOT|0\n")) throw new SecurityException("Root not verified");
        return inventory;
    }
}
