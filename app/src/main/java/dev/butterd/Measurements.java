package dev.butterd;

import java.util.Locale;

/** Pure conversions: invalid or unsupported readings must never become invented zeroes. */
public final class Measurements {
    private Measurements() {}
    public static double batteryWatts(int microamps, int millivolts) {
        if (microamps == Integer.MIN_VALUE || millivolts <= 0 || millivolts > 20000)
            return Double.NaN;
        return microamps / 1_000_000d * millivolts / 1000d;
    }
    public static double sleepPercent(long elapsedDelta, long uptimeDelta) {
        if (elapsedDelta <= 0 || uptimeDelta < 0 || uptimeDelta > elapsedDelta + 50)
            return Double.NaN;
        return Math.max(0, Math.min(100, (elapsedDelta - uptimeDelta) * 100d / elapsedDelta));
    }
    public static double milliCelsius(String raw) {
        try {
            double value = Double.parseDouble(raw.trim()) / 1000d;
            return Double.isFinite(value) && value >= -20 && value <= 150 ? value : Double.NaN;
        } catch (RuntimeException ignored) { return Double.NaN; }
    }
    public static double positiveMicro(String raw, double maximum) {
        try {
            double value = Double.parseDouble(raw.trim()) / 1_000_000d;
            return Double.isFinite(value) && value >= 0 && value <= maximum ? value : Double.NaN;
        } catch (RuntimeException ignored) { return Double.NaN; }
    }
    public static String thermalProfile(String raw) {
        switch (raw.trim()) {
            case "50": return "Balanced";
            case "18": return "Performance";
            case "700": return "Battery saver";
            case "19": return "Gaming";
            default: return "Unknown (" + raw.trim() + ")";
        }
    }
    /** The sign represents net battery flow, even when a charger is connected. */
    public static String signedPower(double watts) {
        if (!Double.isFinite(watts)) return "Unavailable";
        if (Math.abs(watts) < 0.05) return String.format(Locale.getDefault(), "%.1f W", 0d);
        return String.format(Locale.getDefault(), "%+.1f W", watts);
    }
    public static String value(double value, String unit) {
        return Double.isFinite(value) ? String.format(Locale.getDefault(), "%.1f %s", value, unit) : "Unavailable";
    }
    public static String duration(long millis) {
        long minutes = Math.max(0, millis) / 60000;
        return minutes >= 60 ? String.format(Locale.getDefault(), "%dh %02dm", minutes / 60, minutes % 60)
                : minutes + "m";
    }
}
