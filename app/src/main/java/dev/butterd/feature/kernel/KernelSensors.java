package dev.butterd.feature.kernel;

import java.util.Map;

/** Fixed read-only inventory; these paths never become write targets. */
public final class KernelSensors {
    public static final Map<String, String> PATHS = Map.ofEntries(
            Map.entry("cycles", "/sys/class/power_supply/battery/cycle_count"),
            Map.entry("full", "/sys/class/power_supply/battery/charge_full"),
            Map.entry("design", "/sys/class/power_supply/battery/charge_full_design"),
            Map.entry("tcp", "/proc/sys/net/ipv4/tcp_congestion_control"),
            Map.entry("tcp_available", "/proc/sys/net/ipv4/tcp_available_congestion_control"),
            Map.entry("tcp_allowed", "/proc/sys/net/ipv4/tcp_allowed_congestion_control"),
            Map.entry("qdisc", "/proc/sys/net/core/default_qdisc"),
            Map.entry("htsr", "/sys/devices/virtual/touch/touch_dev/bump_sample_rate"),
            Map.entry("thermal_remove", "/sys/class/power_supply/usb/thermal_remove"),
            Map.entry("gpu_margin", "/sys/kernel/ged/hal/timer_base_dvfs_margin"),
            Map.entry("gpu_step", "/sys/kernel/ged/hal/loading_base_dvfs_step"));
    public static String clean(String key, String raw) {
        if (!PATHS.containsKey(key) || raw == null) return "";
        String value = raw.trim();
        if (key.startsWith("tcp") || key.equals("qdisc"))
            return value.matches("[a-zA-Z0-9_ -]{1,160}") ? value : "";
        if (key.equals("htsr") || key.equals("thermal_remove")) return value.matches("[01]") ? value : "";
        return value.matches("[0-9]{1,12}") ? value : "";
    }
    public static double capacityMah(String raw) {
        try { double value = Long.parseLong(raw) / 1000d; return value > 0 && value <= 20000 ? value : Double.NaN; }
        catch (RuntimeException ignored) { return Double.NaN; }
    }
    public static double remainingCapacity(String full, String design) {
        double a = capacityMah(full), b = capacityMah(design);
        double percent = a / b * 100;
        return Double.isFinite(percent) && percent > 0 && percent <= 110 ? percent : Double.NaN;
    }
    private KernelSensors() { }
}
