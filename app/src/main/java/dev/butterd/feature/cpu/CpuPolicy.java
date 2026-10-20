package dev.butterd.feature.cpu;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;

/** Validated kernel capability data. Frequency limits remain owned by the ROM. */
public final class CpuPolicy {
    public final String id, governor, cores, minimum, maximum;
    public final List<String> governors;
    public CpuPolicy(String id, String governor, String available, String cores, String minimum, String maximum) {
        if (!validId(id) || !validGovernor(governor)) throw new IllegalArgumentException("Invalid CPU policy");
        this.id = id; this.governor = governor;
        List<String> values = new ArrayList<>();
        for (String value : available.trim().split("\\s+")) if (validGovernor(value)) values.add(value);
        governors = Collections.unmodifiableList(values);
        this.cores = cores.matches("[0-9 ,\\-]{1,64}") ? cores : "Unavailable";
        this.minimum = mhz(minimum); this.maximum = mhz(maximum);
    }
    public static boolean validId(String value) { return value != null && value.matches("policy[0-9]{1,3}"); }
    public static boolean validGovernor(String value) { return value != null && value.matches("[a-z][a-z0-9_]{0,31}"); }
    public static boolean selectable(String value) {
        return Arrays.asList("schedutil", "ondemand", "conservative", "powersave", "interactive").contains(value);
    }
    public List<String> selectableGovernors() {
        List<String> result = new ArrayList<>();
        for (String value : governors) if (selectable(value)) result.add(value);
        return result;
    }
    private static String mhz(String value) {
        try { long khz = Long.parseLong(value); return khz > 0 && khz < 10000000 ? khz / 1000 + " MHz" : "Unavailable"; }
        catch (RuntimeException ignored) { return "Unavailable"; }
    }
}
