package dev.butterd;

import android.content.ComponentName;
import android.content.Context;
import android.content.Intent;
import android.content.pm.ActivityInfo;
import android.content.pm.PackageManager;
import androidx.core.content.ContextCompat;

/** Opens only verified exported ROM screens. No private preferences or kernel writes. */
final class RomIntegration {
    enum Screen {
        THERMAL("com.xiaomi.settings.thermal.ThermalProfileActivity"),
        PARTS("com.xiaomi.settings.PartsActivity");
        final String activity;
        Screen(String activity) { this.activity = activity; }
    }
    private static Intent intent(Screen screen) {
        return new Intent().setComponent(new ComponentName("com.xiaomi.settings", screen.activity));
    }
    static boolean isAvailable(Context context, Screen screen) {
        try {
            ActivityInfo info = context.getPackageManager().getActivityInfo(intent(screen).getComponent(), 0);
            return info.exported && info.enabled && info.applicationInfo.enabled
                    && (info.permission == null || ContextCompat.checkSelfPermission(context, info.permission) == PackageManager.PERMISSION_GRANTED);
        } catch (PackageManager.NameNotFoundException | SecurityException unavailable) { return false; }
    }
    static boolean open(Context context, Screen screen) {
        if (!isAvailable(context, screen)) return false;
        try { context.startActivity(intent(screen)); return true; }
        catch (android.content.ActivityNotFoundException | SecurityException unavailable) { return false; }
    }
}
