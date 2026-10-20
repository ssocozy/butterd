package dev.butterd.feature.display;

import java.util.Objects;

/** Original strings are retained so an absent preference can be restored by deletion. */
public final class RefreshPreference {
    public final String minimum, peak;
    public RefreshPreference(String minimum, String peak) {
        if (!validStored(minimum) || !validStored(peak)) throw new IllegalArgumentException("Unrecognized display preference");
        this.minimum = minimum; this.peak = peak;
    }
    public static boolean validStored(String value) {
        if ("null".equals(value)) return true;
        if (value == null || !value.matches("[0-9]{1,4}(\\.[0-9]{1,9})?")) return false;
        try { double n = Double.parseDouble(value); return Double.isFinite(n) && n >= 0 && n <= 1000; }
        catch (NumberFormatException ignored) { return false; }
    }
    public static boolean supported(float requested, float[] rates) {
        if (!Float.isFinite(requested) || requested < 24 || requested > 240) return false;
        for (float rate : rates) if (Math.abs(rate - requested) < 0.01f) return true;
        return false;
    }
    @Override public boolean equals(Object other) {
        if (!(other instanceof RefreshPreference)) return false;
        RefreshPreference p = (RefreshPreference) other;
        return minimum.equals(p.minimum) && peak.equals(p.peak);
    }
    @Override public int hashCode() { return Objects.hash(minimum, peak); }
}
