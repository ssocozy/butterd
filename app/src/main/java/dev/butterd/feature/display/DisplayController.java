package dev.butterd.feature.display;

import android.content.Context;
import android.content.SharedPreferences;
import android.hardware.display.DisplayManager;
import android.os.Process;
import android.view.Display;
import dev.butterd.core.root.RootExecutor;
import java.util.ArrayList;
import java.util.List;

/** Explicit global refresh preferences. No display resolution, thermal or boot-service changes. */
public final class DisplayController {
    private final Context context;
    private final SharedPreferences journal;
    private final int user;
    public DisplayController(Context context) {
        this.context = context.getApplicationContext();
        journal = context.getSharedPreferences("display_originals", Context.MODE_PRIVATE);
        // Android partitions UIDs into 100000-wide user ranges. Scope writes to this app's user.
        user = Process.myUid() / 100000;
    }
    public boolean hasOriginal() { return journal.contains("original.min"); }
    public float[] supportedRates() {
        Display display = context.getSystemService(DisplayManager.class).getDisplay(Display.DEFAULT_DISPLAY);
        if (display == null) return new float[0];
        Display.Mode current = display.getMode(); List<Float> rates = new ArrayList<>();
        for (Display.Mode mode : display.getSupportedModes()) {
            if (mode.getPhysicalWidth() != current.getPhysicalWidth() || mode.getPhysicalHeight() != current.getPhysicalHeight()) continue;
            float rate = mode.getRefreshRate();
            if (Float.isFinite(rate) && rate >= 24 && rate <= 240 && !rates.contains(rate)) rates.add(rate);
        }
        java.util.Collections.sort(rates);
        float[] result = new float[rates.size()]; for (int i = 0; i < result.length; i++) result[i] = rates.get(i);
        return result;
    }
    public void apply(float hz, boolean adaptive, boolean restore) throws Exception {
        if (!restore && !RefreshPreference.supported(hz, supportedRates())) throw new IllegalArgumentException("Rate is not supported at the current resolution");
        RefreshPreference live = read();
        if (restore && hasOriginal() && live.equals(saved("original"))) {
            if (!journal.edit().clear().commit()) throw new IllegalStateException("Could not clear the restored display record");
            return;
        }
        if (hasOriginal() && !live.equals(saved("before")) && !live.equals(saved("after")))
            throw new IllegalStateException("Display preferences changed outside Butterd. No values were overwritten.");
        if (restore && !hasOriginal()) throw new IllegalStateException("No saved display preferences");
        RefreshPreference target = restore ? saved("original") : new RefreshPreference(adaptive ? "0" : Float.toString(hz), Float.toString(hz));
        if (live.equals(target)) return;
        if (!hasOriginal()) {
            SharedPreferences.Editor e = journal.edit(); put(e, "original", live); put(e, "before", live); put(e, "after", live);
            if (!e.commit()) throw new IllegalStateException("Could not save originals; no display change made");
        }
        // Relax the minimum before lowering the peak, then apply the requested floor.
        // Each step saves both possible outcomes before writing, so Restore can recover
        // a process interruption without accepting arbitrary external state.
        live = transition(live, new RefreshPreference("0", live.peak), "min_refresh_rate", "0");
        live = transition(live, new RefreshPreference(live.minimum, target.peak), "peak_refresh_rate", target.peak);
        transition(live, target, "min_refresh_rate", target.minimum);
        if (restore && !journal.edit().clear().commit()) throw new IllegalStateException("Display restored, but its saved record could not be cleared");
    }
    private RefreshPreference transition(RefreshPreference from, RefreshPreference to, String key, String value) throws Exception {
        if (from.equals(to)) return to;
        SharedPreferences.Editor editor = journal.edit(); put(editor, "before", from); put(editor, "after", to);
        if (!editor.commit()) throw new IllegalStateException("Could not journal the display transition");
        String script = "[ \"$(" + get("min_refresh_rate") + ")\" = '" + from.minimum + "' ] || exit 2; "
                + "[ \"$(" + get("peak_refresh_rate") + ")\" = '" + from.peak + "' ] || exit 2; "
                + (value.equals("null") ? settings() + "delete system " + key : settings() + "put system " + key + " '" + value + "'")
                + " >/dev/null || exit 3; "
                + "[ \"$(" + get("min_refresh_rate") + ")\" = '" + to.minimum + "' ] || exit 4; "
                + "[ \"$(" + get("peak_refresh_rate") + ")\" = '" + to.peak + "' ] || exit 4; printf VERIFIED";
        if (!"VERIFIED".equals(RootExecutor.run(script, 10, 1024))) throw new IllegalStateException("Display readback failed; original preferences retained");
        return to;
    }
    private RefreshPreference read() throws Exception {
        String output = RootExecutor.run("printf '%s|%s' \"$(" + get("min_refresh_rate") + ")\" \"$(" + get("peak_refresh_rate") + ")\"", 10, 1024);
        String[] pair = output.split("\\|", -1);
        if (pair.length != 2) throw new IllegalStateException("Display preferences could not be read");
        return new RefreshPreference(pair[0], pair[1]);
    }
    private String settings() { return "/system/bin/settings --user " + user + " "; }
    private String get(String key) { return settings() + "get system " + key; }
    private RefreshPreference saved(String prefix) {
        return new RefreshPreference(journal.getString(prefix + ".min", ""), journal.getString(prefix + ".peak", ""));
    }
    private static void put(SharedPreferences.Editor e, String prefix, RefreshPreference value) {
        e.putString(prefix + ".min", value.minimum).putString(prefix + ".peak", value.peak);
    }
}
