package dev.butterd;

import android.Manifest;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.content.Intent;
import android.content.SharedPreferences;
import android.content.pm.PackageManager;
import android.content.res.ColorStateList;
import android.content.res.Configuration;
import android.os.Build;
import android.os.Bundle;
import android.provider.Settings;
import android.view.View;
import android.view.ViewGroup;
import android.view.ViewStub;
import android.widget.LinearLayout;
import android.content.ClipData;
import androidx.core.content.FileProvider;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import android.widget.TextView;
import androidx.annotation.NonNull;
import androidx.appcompat.app.AppCompatActivity;
import androidx.appcompat.app.AppCompatDelegate;
import androidx.core.app.NotificationManagerCompat;
import androidx.core.content.ContextCompat;
import androidx.core.graphics.Insets;
import androidx.core.view.ViewCompat;
import androidx.core.view.WindowCompat;
import androidx.core.view.WindowInsetsCompat;
import com.google.android.material.button.MaterialButton;
import com.google.android.material.button.MaterialButtonToggleGroup;
import com.google.android.material.materialswitch.MaterialSwitch;
import com.google.android.material.snackbar.Snackbar;
import java.util.Locale;

public final class MainActivity extends AppCompatActivity implements TelemetryRepository.Listener {
    private TelemetryRepository repository;
    private SharedPreferences prefs;
    private MaterialSwitch notificationSwitch;
    private MaterialButton rootButton;
    private String lastError = "";
    private boolean rootRequested, syncing, pendingRecording, cpuBusy;
    private Snapshot currentSnapshot;
    private String cpuSignature = "", lastRecordingError = "";
    private int page;
    private long lastGraphTime;
    private final int[] pages = {R.id.page_overview, R.id.page_device, R.id.page_settings, R.id.page_cpu, R.id.page_battery, R.id.page_gpu};
    private final int[] navigation = {R.id.nav_overview, R.id.nav_device, R.id.nav_settings};
    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        WindowCompat.setDecorFitsSystemWindows(getWindow(), false);
        setContentView(R.layout.activity_main);
        prefs = getSharedPreferences("ui", MODE_PRIVATE);
        repository = ((ButterdApp) getApplication()).repository();
        boolean dark = (getResources().getConfiguration().uiMode & Configuration.UI_MODE_NIGHT_MASK) == Configuration.UI_MODE_NIGHT_YES;
        WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView()).setAppearanceLightStatusBars(!dark);
        WindowCompat.getInsetsController(getWindow(), getWindow().getDecorView()).setAppearanceLightNavigationBars(!dark);
        ViewCompat.setOnApplyWindowInsetsListener(findViewById(R.id.root), (view, insets) -> {
            Insets bars = insets.getInsets(WindowInsetsCompat.Type.systemBars() | WindowInsetsCompat.Type.displayCutout());
            view.setPadding(bars.left, bars.top, bars.right, 0);
            View bar = findViewById(R.id.bottom_bar);
            ViewGroup.MarginLayoutParams params = (ViewGroup.MarginLayoutParams) bar.getLayoutParams();
            params.bottomMargin = bars.bottom + dp(12); bar.setLayoutParams(params);
            for (int id : pages) findViewById(id).setPadding(0, 0, 0, bars.bottom);
            return insets;
        });
        for (int i = 0; i < navigation.length; i++) {
            final int tab = i; findViewById(navigation[i]).setOnClickListener(view -> selectPage(tab));
        }
        pendingRecording = state != null && state.getBoolean("pending_recording", false);
        findViewById(R.id.home_cpu).setOnClickListener(view -> selectPage(3));
        findViewById(R.id.home_battery).setOnClickListener(view -> selectPage(4));
        findViewById(R.id.home_gpu).setOnClickListener(view -> selectPage(5));
        selectPage(state == null ? 0 : state.getInt("page", 0));
        text(R.id.model, Build.MANUFACTURER + " " + Build.MODEL + " · " + Build.DEVICE);
        text(R.id.rom, "Android " + Build.VERSION.RELEASE + " · " + System.getProperty("os.version", "Unknown"));
        findViewById(R.id.rom_thermal).setOnClickListener(view -> openRomScreen(RomIntegration.Screen.THERMAL));
        findViewById(R.id.rom_parts).setOnClickListener(view -> openRomScreen(RomIntegration.Screen.PARTS));
        refreshRomScreens();
        rootButton = findViewById(R.id.root_button);
        rootButton.setOnClickListener(view -> {
            if (repository.isRootEnabled()) { repository.disableRoot(); message(getString(R.string.root_disabled)); }
            else {
                rootRequested = true; rootButton.setEnabled(false); rootButton.setText(R.string.root_checking);
                message(getString(R.string.root_checking)); repository.requestRoot();
            }
        });
        notificationSwitch = findViewById(R.id.notification_switch);
        notificationSwitch.setOnCheckedChangeListener((button, checked) -> {
            if (syncing) return;
            if (!checked) { stopService(new Intent(this, MonitorService.class)); message(getString(R.string.notification_stopped)); return; }
            requestMonitor(false);
        });
        MaterialButtonToggleGroup themes = findViewById(R.id.theme_group);
        int current = prefs.getInt("theme", AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM);
        themes.check(current == AppCompatDelegate.MODE_NIGHT_YES ? R.id.theme_dark : current == AppCompatDelegate.MODE_NIGHT_NO ? R.id.theme_light : R.id.theme_system);
        themes.addOnButtonCheckedListener((group, checkedId, checked) -> {
            if (!checked) return;
            int mode = checkedId == R.id.theme_dark ? AppCompatDelegate.MODE_NIGHT_YES : checkedId == R.id.theme_light ? AppCompatDelegate.MODE_NIGHT_NO : AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM;
            prefs.edit().putInt("theme", mode).apply(); AppCompatDelegate.setDefaultNightMode(mode);
        });
        MaterialSwitch solid = findViewById(R.id.solid_switch);
        solid.setChecked(prefs.getBoolean("solid", false)); setSolid(solid.isChecked());
        solid.setOnCheckedChangeListener((button, checked) -> { prefs.edit().putBoolean("solid", checked).apply(); setSolid(checked); });
        getOnBackPressedDispatcher().addCallback(this, new androidx.activity.OnBackPressedCallback(true) {
            @Override public void handleOnBackPressed() {
                if (page > 2) selectPage(0);
                else if (page != 0) selectPage(0);
                else finish();
            }
        });
        ViewCompat.requestApplyInsets(findViewById(R.id.root));
    }
    @Override protected void onStart() { super.onStart(); repository.add(this); syncNotification(); }
    @Override protected void onStop() { repository.remove(this); super.onStop(); }
    @Override protected void onResume() { super.onResume(); if (notificationSwitch != null) { syncNotification(); refreshRomScreens(); } }
    @Override protected void onSaveInstanceState(@NonNull Bundle state) { state.putInt("page", page); state.putBoolean("pending_recording", pendingRecording); super.onSaveInstanceState(state); }
    private void refreshRomScreens() {
        boolean thermal = RomIntegration.isAvailable(this, RomIntegration.Screen.THERMAL);
        boolean parts = RomIntegration.isAvailable(this, RomIntegration.Screen.PARTS);
        findViewById(R.id.rom_thermal).setEnabled(thermal);
        findViewById(R.id.rom_parts).setEnabled(parts);
        text(R.id.rom_controls_note, getString(thermal || parts ? R.string.rom_controls_note : R.string.rom_controls_missing));
    }
    private void openRomScreen(RomIntegration.Screen screen) {
        if (!RomIntegration.open(this, screen)) {
            refreshRomScreens(); message(getString(R.string.rom_screen_unavailable));
        }
    }
    private void setSolid(boolean solid) { findViewById(R.id.bottom_bar).setBackgroundResource(solid ? R.drawable.solid_nav : R.drawable.glass); }
    private void selectPage(int selected) {
        page = Math.max(0, Math.min(pages.length - 1, selected));
        View target = findViewById(pages[page]);
        if (target instanceof ViewStub) { ((ViewStub) target).inflate(); setupContextPage(page); }
        int activeTab = page > 2 ? 1 : page;
        for (int i = 0; i < pages.length; i++) {
            findViewById(pages[i]).setVisibility(i == page ? View.VISIBLE : View.GONE);
        }
        for (int i = 0; i < navigation.length; i++) {
            MaterialButton button = findViewById(navigation[i]); button.setSelected(i == activeTab);
            button.setBackgroundTintList(ColorStateList.valueOf(ContextCompat.getColor(this, i == activeTab ? R.color.chip : android.R.color.transparent)));
            button.setIconTint(ColorStateList.valueOf(ContextCompat.getColor(this, R.color.ink)));
            ViewCompat.setStateDescription(button, i == activeTab ? "Selected" : null);
        }
        ViewCompat.requestApplyInsets(findViewById(R.id.root));
        if (currentSnapshot != null) renderContext(currentSnapshot);
    }
    private void requestMonitor(boolean forRecording) {
        pendingRecording = forRecording;
        if (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) {
            if (prefs.getBoolean("notification_requested", false) && !shouldShowRequestPermissionRationale(Manifest.permission.POST_NOTIFICATIONS)) {
                pendingRecording = false; syncNotification(); notificationSettingsMessage(); return;
            }
            prefs.edit().putBoolean("notification_requested", true).apply();
            requestPermissions(new String[]{Manifest.permission.POST_NOTIFICATIONS}, 10);
        } else startMonitor();
    }
    private void startMonitor() {
        NotificationChannel channel = getSystemService(NotificationManager.class).getNotificationChannel(MonitorService.CHANNEL);
        if (!NotificationManagerCompat.from(this).areNotificationsEnabled() || (channel != null && channel.getImportance() == NotificationManager.IMPORTANCE_NONE)) {
            pendingRecording = false; syncNotification(); notificationSettingsMessage(); return;
        }
        boolean recordingRequested = pendingRecording; pendingRecording = false;
        try {
            Intent start = new Intent(this, MonitorService.class);
            if (recordingRequested) start.setAction(MonitorService.START_LOG);
            ContextCompat.startForegroundService(this, start);
            notificationSwitch.postDelayed(() -> {
                if (isFinishing() || isDestroyed()) return;
                syncNotification(); message(getString(MonitorService.isRunning() ? R.string.notification_started : R.string.monitor_failed));
            }, 700);
        } catch (RuntimeException failure) { syncNotification(); message(getString(R.string.monitor_failed)); }
    }
    private void syncNotification() {
        syncing = true; notificationSwitch.setChecked(MonitorService.isRunning()); syncing = false;
    }
    private void notificationSettingsMessage() {
        Snackbar.make(findViewById(R.id.root), R.string.notification_denied, Snackbar.LENGTH_LONG)
                .setAnchorView(R.id.bottom_bar).setAction(R.string.open_settings, v -> {
                    Intent intent = new Intent(Settings.ACTION_APP_NOTIFICATION_SETTINGS).putExtra(Settings.EXTRA_APP_PACKAGE, getPackageName()); startActivity(intent);
                }).show();
    }
    @Override public void onRequestPermissionsResult(int code, @NonNull String[] permissions, @NonNull int[] results) {
        super.onRequestPermissionsResult(code, permissions, results);
        if (code == 10) {
            if (results.length > 0 && results[0] == PackageManager.PERMISSION_GRANTED) startMonitor();
            else { pendingRecording = false; syncNotification(); notificationSettingsMessage(); }
        }
    }
    @Override public void onSnapshot(Snapshot s) {
        currentSnapshot = s;
        text(R.id.power, Double.isFinite(s.watts) ? String.format(Locale.getDefault(), "%.1f W", Math.abs(s.watts)) : "—");
        text(R.id.power_direction, !Double.isFinite(s.watts) ? getString(R.string.unavailable) : s.watts > 0 ? "Flowing into the battery" : s.watts < 0 ? "Drawing from the battery" : "No net battery flow reported");
        text(R.id.battery_state, (s.capacity >= 0 ? s.capacity + "%" : "—") + "  ·  " + (s.charging ? "Charging" : s.plugged ? "Plugged in" : "On battery"));
        text(R.id.battery_temp, Measurements.value(s.batteryTemperature, "°C"));
        text(R.id.battery_voltage, Measurements.value(s.voltage, "V"));
        text(R.id.sleep, Double.isFinite(s.sleepPercent) ? String.format(Locale.getDefault(), "%.0f%%", s.sleepPercent) : "—");
        text(R.id.sleep_note, getString(R.string.sleep_note) + " Suspended: " + Measurements.duration(s.sleepingMillis) + ".");
        text(R.id.thermal, s.thermal);
        text(R.id.cpu_temp, Measurements.value(s.cpuTemperature, "°C") + (s.cpuSource.isEmpty() ? "" : " · " + s.cpuSource));
        text(R.id.cpu_freq, s.frequencies); text(R.id.gpu_freq, s.gpuFrequency); text(R.id.profile, s.profile);
        text(R.id.adapter_voltage, Measurements.value(s.usbVoltage, "V")); text(R.id.adapter_current, Measurements.value(s.usbCurrent, "A"));
        text(R.id.root_state, getString(s.root ? R.string.root_active : R.string.root_standard));
        rootButton.setEnabled(!s.rootBusy);
        rootButton.setText(s.rootBusy ? R.string.root_checking : s.root ? R.string.disconnect_root : R.string.request_root);
        if (rootRequested && !s.rootBusy) {
            rootRequested = false; message(getString(s.root ? R.string.root_granted : R.string.root_denied));
        } else if (!s.error.isEmpty() && !s.error.equals(lastError)) message(s.error);
        lastError = s.error;
        if (s.elapsed != lastGraphTime) { ((PowerGraph) findViewById(R.id.graph)).add(s.watts); lastGraphTime = s.elapsed; }
        renderContext(s);
        if (!s.recordingError.isEmpty() && !s.recordingError.equals(lastRecordingError)) message(s.recordingError);
        lastRecordingError = s.recordingError;
        syncNotification();
    }
    private void setupContextPage(int selected) {
        if (selected == 3) findViewById(R.id.cpu_access).setOnClickListener(view -> selectPage(1));
        if (selected == 4) {
            findViewById(R.id.battery_rom).setOnClickListener(view -> openRomScreen(RomIntegration.Screen.PARTS));
            findViewById(R.id.record_button).setOnClickListener(view -> {
                if (repository.isRecording()) { repository.stopRecording(); message(getString(R.string.record_stopped)); }
                else new MaterialAlertDialogBuilder(this).setTitle(R.string.record_start)
                        .setMessage(R.string.record_privacy).setNegativeButton(android.R.string.cancel, null)
                        .setPositiveButton(R.string.record_start_action, (dialog, which) -> requestMonitor(true)).show();
            });
            findViewById(R.id.marker_button).setOnClickListener(view -> {
                String[] markers = {"Before changing ROM settings", "Lower charging preset selected in ROM", "Bypass enabled in ROM", "Bypass disabled in ROM", "Battery behavior observed"};
                new MaterialAlertDialogBuilder(this).setTitle(R.string.record_marker).setItems(markers, (dialog, which) -> {
                    repository.marker(markers[which], error -> {
                        if (!isFinishing() && !isDestroyed()) message(error.isEmpty() ? getString(R.string.record_marker_saved) : error);
                    });
                }).show();
            });
            findViewById(R.id.share_button).setOnClickListener(view -> repository.exportLog((file, error) -> {
                if (isFinishing() || isDestroyed()) return;
                if (!error.isEmpty()) { message(error); return; }
                try {
                    android.net.Uri uri = FileProvider.getUriForFile(this, getPackageName() + ".files", file);
                    Intent share = new Intent(Intent.ACTION_SEND).setType("text/plain").putExtra(Intent.EXTRA_STREAM, uri)
                            .addFlags(Intent.FLAG_GRANT_READ_URI_PERMISSION);
                    share.setClipData(ClipData.newRawUri("Butterd diagnostics", uri));
                    startActivity(Intent.createChooser(share, getString(R.string.share_recording)));
                } catch (RuntimeException unavailable) { message("Could not open the share sheet. The recording remains saved."); }
            }));
        }
    }
    private void renderContext(Snapshot s) {
        text(R.id.cpu_online, s.onlineCores);
        text(R.id.detail_battery_power, Measurements.value(s.watts, "W"));
        text(R.id.detail_current, Measurements.value(s.currentAmps, "A"));
        text(R.id.detail_power_path, s.powerPath);
        text(R.id.detail_voltage, Measurements.value(s.voltage, "V"));
        text(R.id.detail_temp, Measurements.value(s.batteryTemperature, "°C"));
        text(R.id.detail_usb_voltage, Measurements.value(s.usbVoltage, "V"));
        text(R.id.detail_usb_current, Measurements.value(s.usbCurrent, "A"));
        text(R.id.detail_charge_limit, Measurements.value(s.chargeLimitAmps, "A"));
        text(R.id.detail_bypass, s.bypassCommand);
        text(R.id.detail_charging_source, s.chargingSource + (s.chargingSampleAge < 0 ? "" : " · sample age " + s.chargingSampleAge / 1000 + " s"));
        text(R.id.detail_gpu_freq, s.gpuFrequency);
        text(R.id.record_status, s.logStatus);
        MaterialButton record = findViewById(R.id.record_button);
        if (record != null) { record.setText(s.recording ? R.string.record_stop : R.string.record_start); findViewById(R.id.marker_button).setEnabled(s.recording); }
        LinearLayout list = findViewById(R.id.cpu_policies);
        if (list == null) return;
        StringBuilder signature = new StringBuilder().append(s.root).append(cpuBusy);
        for (CpuPolicy p : s.policies) signature.append(p.id).append(p.governor).append(p.governors).append(p.cores).append(p.minimum).append(p.maximum).append(repository.hasCpuOriginal(p.id));
        if (signature.toString().equals(cpuSignature)) return;
        cpuSignature = signature.toString(); list.removeAllViews();
        if (s.policies.isEmpty()) {
            TextView unavailable = new TextView(this); unavailable.setText(R.string.cpu_unavailable);
            unavailable.setTextColor(ContextCompat.getColor(this, R.color.secondary)); unavailable.setPadding(0, dp(20), 0, 0); list.addView(unavailable);
        }
        for (CpuPolicy policy : s.policies) {
            View row = getLayoutInflater().inflate(R.layout.cpu_policy, list, false);
            ((TextView) row.findViewById(R.id.policy_title)).setText(policy.id + " · cores " + policy.cores);
            ((TextView) row.findViewById(R.id.policy_details)).setText("Governor: " + policy.governor + "\nROM frequency range: " + policy.minimum + " – " + policy.maximum + "\nAvailable: " + String.join(", ", policy.governors));
            MaterialButton choose = row.findViewById(R.id.policy_governor), restore = row.findViewById(R.id.policy_restore);
            choose.setEnabled(s.root && !cpuBusy && !policy.selectableGovernors().isEmpty());
            restore.setEnabled(s.root && !cpuBusy && repository.hasCpuOriginal(policy.id));
            choose.setOnClickListener(view -> {
                String[] choices = policy.selectableGovernors().toArray(new String[0]);
                new MaterialAlertDialogBuilder(this).setTitle(policy.id + " · " + getString(R.string.cpu_select))
                        .setItems(choices, (dialog, which) -> new MaterialAlertDialogBuilder(this)
                                .setTitle(choices[which]).setMessage(R.string.cpu_confirm).setNegativeButton(android.R.string.cancel, null)
                                .setPositiveButton(android.R.string.ok, (confirmation, ok) -> changeGovernor(policy.id, choices[which], false)).show()).show();
            });
            restore.setOnClickListener(view -> changeGovernor(policy.id, "", true)); list.addView(row);
        }
    }
    private void changeGovernor(String policy, String governor, boolean restore) {
        cpuBusy = true; if (currentSnapshot != null) renderContext(currentSnapshot);
        message(getString(R.string.cpu_pending));
        repository.changeGovernor(policy, governor, restore, error -> {
            cpuBusy = false; cpuSignature = "";
            if (isFinishing() || isDestroyed()) return;
            if (currentSnapshot != null) renderContext(currentSnapshot);
            message(error.isEmpty() ? getString(restore ? R.string.cpu_restored : R.string.cpu_changed) : error);
        });
    }
    private void text(int id, String value) {
        TextView view = findViewById(id); if (view != null && !value.contentEquals(view.getText())) view.setText(value);
    }
    private void message(String message) { Snackbar.make(findViewById(R.id.root), message, Snackbar.LENGTH_LONG).setAnchorView(R.id.bottom_bar).show(); }
    private int dp(int value) { return Math.round(value * getResources().getDisplayMetrics().density); }
}
