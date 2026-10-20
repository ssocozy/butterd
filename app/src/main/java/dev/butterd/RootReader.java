package dev.butterd;

import java.io.BufferedReader;
import java.io.InputStreamReader;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.TimeUnit;

/** No arbitrary command API. Only this fixed, bounded sensor inventory can run as root. */
final class RootReader {
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

    static String read() throws Exception {
        Process process = new ProcessBuilder("su", "-c", SCRIPT).redirectErrorStream(true).start();
        try {
            if (!process.waitFor(20, TimeUnit.SECONDS)) throw new IllegalStateException("Root request timed out");
            if (process.exitValue() != 0) throw new SecurityException("Root unavailable");
            StringBuilder out = new StringBuilder();
            try (BufferedReader reader = new BufferedReader(new InputStreamReader(process.getInputStream(), StandardCharsets.UTF_8))) {
                String line;
                while ((line = reader.readLine()) != null && out.length() < 16384) out.append(line).append('\n');
            }
            if (!out.toString().contains("ROOT|0\n")) throw new SecurityException("Root not verified");
            return out.toString();
        } finally { process.destroy(); }
    }
}
