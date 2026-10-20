package dev.butterd;

import android.Manifest;
import android.app.Instrumentation;
import android.content.Intent;
import android.graphics.Bitmap;
import android.os.ParcelFileDescriptor;
import androidx.appcompat.app.AppCompatDelegate;
import androidx.test.core.app.ActivityScenario;
import androidx.test.ext.junit.runners.AndroidJUnit4;
import androidx.test.platform.app.InstrumentationRegistry;
import com.google.android.material.materialswitch.MaterialSwitch;
import org.junit.Test;
import org.junit.runner.RunWith;
import java.io.File;
import java.io.FileOutputStream;
import static org.junit.Assert.*;

@RunWith(AndroidJUnit4.class)
public class AppSmokeTest {
    private final Instrumentation instrumentation = InstrumentationRegistry.getInstrumentation();
    @Test public void defaultLaunchIsReadOnlyAndNavigationAndThemesWork() throws Exception {
        try (ActivityScenario<MainActivity> scenario = ActivityScenario.launch(MainActivity.class)) {
            instrumentation.waitForIdleSync();
            scenario.onActivity(activity -> {
                assertFalse(MonitorService.isRunning());
                assertFalse(((ButterdApp) activity.getApplication()).repository().isRootEnabled());
                assertFalse(((MaterialSwitch) activity.findViewById(R.id.notification_switch)).isChecked());
                activity.findViewById(R.id.nav_device).performClick();
                assertEquals(android.view.View.VISIBLE, activity.findViewById(R.id.page_device).getVisibility());
                activity.findViewById(R.id.nav_settings).performClick();
                activity.findViewById(R.id.theme_light).performClick();
            });
            instrumentation.waitForIdleSync();
            scenario.onActivity(activity -> activity.findViewById(R.id.nav_overview).performClick());
            screenshot("overview-white");
            scenario.onActivity(activity -> { activity.findViewById(R.id.nav_settings).performClick(); activity.findViewById(R.id.theme_dark).performClick(); });
            instrumentation.waitForIdleSync();
            scenario.onActivity(activity -> activity.findViewById(R.id.nav_overview).performClick());
            screenshot("overview-black");
            scenario.onActivity(activity -> { activity.findViewById(R.id.nav_device).performClick(); });
            screenshot("device-black");
            scenario.onActivity(activity -> { activity.findViewById(R.id.nav_overview).performClick(); activity.findViewById(R.id.home_cpu).performClick(); });
            screenshot("cpu-black");
            scenario.onActivity(activity -> { activity.findViewById(R.id.nav_overview).performClick(); activity.findViewById(R.id.home_battery).performClick(); });
            screenshot("battery-black");
            scenario.onActivity(activity -> { activity.findViewById(R.id.nav_overview).performClick(); activity.findViewById(R.id.home_gpu).performClick(); });
            screenshot("gpu-black");
            scenario.onActivity(activity -> activity.findViewById(R.id.nav_settings).performClick());
            screenshot("settings-black");
            scenario.recreate();
            scenario.onActivity(activity -> assertEquals(android.view.View.VISIBLE, activity.findViewById(R.id.page_settings).getVisibility()));
        }
    }
    @Test public void deniedNotificationsDoNotStartMonitorAndGrantedMonitorStops() throws Exception {
        String pkg = instrumentation.getTargetContext().getPackageName();
        shell("pm revoke " + pkg + " " + Manifest.permission.POST_NOTIFICATIONS);
        try (ActivityScenario<MainActivity> scenario = ActivityScenario.launch(MainActivity.class)) {
            scenario.onActivity(activity -> {
                // Background service must also reject denied notifications, independent of UI.
                activity.startForegroundService(new Intent(activity, MonitorService.class));
            });
            Thread.sleep(1000);
            assertFalse(MonitorService.isRunning());
            shell("pm grant " + pkg + " " + Manifest.permission.POST_NOTIFICATIONS);
            scenario.onActivity(activity -> {
                activity.findViewById(R.id.nav_settings).performClick();
                ((MaterialSwitch) activity.findViewById(R.id.notification_switch)).setChecked(true);
            });
            Thread.sleep(1500);
            assertTrue(MonitorService.isRunning());
            screenshot("notification-enabled");
            scenario.onActivity(activity -> activity.stopService(new Intent(activity, MonitorService.class)));
            Thread.sleep(500);
            assertFalse(MonitorService.isRunning());
        } finally {
            instrumentation.getTargetContext().stopService(new Intent(instrumentation.getTargetContext(), MonitorService.class));
        }
    }
    private void shell(String command) throws Exception {
        try (ParcelFileDescriptor descriptor = instrumentation.getUiAutomation().executeShellCommand(command);
             java.io.FileInputStream stream = new java.io.FileInputStream(descriptor.getFileDescriptor())) {
            byte[] buffer = new byte[1024]; while (stream.read(buffer) != -1) { }
        }
    }
    private void screenshot(String name) throws Exception {
        instrumentation.waitForIdleSync();
        Bitmap bitmap = instrumentation.getUiAutomation().takeScreenshot();
        assertNotNull(bitmap);
        File dir = new File(instrumentation.getTargetContext().getExternalFilesDir(null), "screenshots");
        assertTrue(dir.isDirectory() || dir.mkdirs());
        try (FileOutputStream out = new FileOutputStream(new File(dir, name + ".png"))) { bitmap.compress(Bitmap.CompressFormat.PNG, 100, out); }
        bitmap.recycle();
    }
}
