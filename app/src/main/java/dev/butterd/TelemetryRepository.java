package dev.butterd;

import android.content.Context;
import android.content.Intent;
import android.content.IntentFilter;
import android.os.BatteryManager;
import android.os.Build;
import android.os.Handler;
import android.os.HandlerThread;
import android.os.Looper;
import android.os.PowerManager;
import android.os.SystemClock;
import java.io.BufferedReader;
import java.io.File;
import java.io.FileReader;
import java.util.ArrayList;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.CopyOnWriteArraySet;

/** One process-wide sampler. File IO and su always execute on a worker thread. */
final class TelemetryRepository {
    interface Listener { void onSnapshot(Snapshot snapshot); }
    private final Context context;
    private final Handler worker, main = new Handler(Looper.getMainLooper());
    private final CopyOnWriteArraySet<Listener> listeners = new CopyOnWriteArraySet<>();
    private final long startElapsed = SystemClock.elapsedRealtime(), startUptime = SystemClock.uptimeMillis();
    private volatile boolean rootEnabled, rootBusy;
    private volatile Snapshot latest;
    private String rootData = "", rootError = "";
    private long lastRootRead;
    private final Runnable tick = new Runnable() {
        @Override public void run() {
            if (listeners.isEmpty()) return;
            sample();
            PowerManager power = (PowerManager) context.getSystemService(Context.POWER_SERVICE);
            worker.postDelayed(this, power.isInteractive() ? 3000 : 15000);
        }
    };
    TelemetryRepository(Context context) {
        this.context = context.getApplicationContext();
        HandlerThread thread = new HandlerThread("butterd-sensors", android.os.Process.THREAD_PRIORITY_BACKGROUND);
        thread.start(); worker = new Handler(thread.getLooper());
    }
    void add(Listener listener) {
        listeners.add(listener);
        if (latest != null) main.post(() -> { if (listeners.contains(listener)) listener.onSnapshot(latest); });
        worker.post(() -> { worker.removeCallbacks(tick); worker.post(tick); });
    }
    void remove(Listener listener) {
        listeners.remove(listener);
        if (listeners.isEmpty()) worker.removeCallbacks(tick);
    }
    boolean isRootEnabled() { return rootEnabled; }
    boolean isRootBusy() { return rootBusy; }
    void requestRoot() {
        if (rootBusy) return;
        rootBusy = true;
        worker.post(() -> {
            try { rootData = RootReader.read(); rootEnabled = true; rootError = ""; lastRootRead = SystemClock.elapsedRealtime(); }
            catch (Exception failure) { rootEnabled = false; rootData = ""; rootError = context.getString(R.string.root_denied); }
            finally { rootBusy = false; sample(); }
        });
    }
    void disableRoot() {
        rootEnabled = false;
        worker.post(() -> { rootData = ""; rootError = ""; lastRootRead = 0; sample(); });
    }
    private void sample() {
        Snapshot s = new Snapshot();
        s.elapsed = SystemClock.elapsedRealtime();
        long elapsed = s.elapsed - startElapsed, awake = SystemClock.uptimeMillis() - startUptime;
        s.sleepPercent = Measurements.sleepPercent(elapsed, awake);
        s.sleepingMillis = Math.max(0, elapsed - awake);
        try {
            Intent battery = context.registerReceiver(null, new IntentFilter(Intent.ACTION_BATTERY_CHANGED));
            if (battery != null && battery.getBooleanExtra(BatteryManager.EXTRA_PRESENT, true)) {
                int scale = battery.getIntExtra(BatteryManager.EXTRA_SCALE, -1);
                int level = battery.getIntExtra(BatteryManager.EXTRA_LEVEL, -1);
                if (level >= 0 && scale > 0 && level <= scale) s.capacity = Math.round(level * 100f / scale);
                int voltage = battery.getIntExtra(BatteryManager.EXTRA_VOLTAGE, -1);
                if (voltage > 0 && voltage <= 20000) s.voltage = voltage / 1000d;
                if (battery.hasExtra(BatteryManager.EXTRA_TEMPERATURE)) {
                    double temp = battery.getIntExtra(BatteryManager.EXTRA_TEMPERATURE, Integer.MIN_VALUE) / 10d;
                    if (temp >= -20 && temp <= 100) s.batteryTemperature = temp;
                }
                s.plugged = battery.getIntExtra(BatteryManager.EXTRA_PLUGGED, 0) != 0;
                s.charging = battery.getIntExtra(BatteryManager.EXTRA_STATUS, -1) == BatteryManager.BATTERY_STATUS_CHARGING;
                BatteryManager manager = (BatteryManager) context.getSystemService(Context.BATTERY_SERVICE);
                s.watts = Measurements.batteryWatts(manager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CURRENT_NOW), voltage);
            }
            if (Build.VERSION.SDK_INT >= 29) {
                int status = ((PowerManager) context.getSystemService(Context.POWER_SERVICE)).getCurrentThermalStatus();
                String[] names = {"None", "Light", "Moderate", "Severe", "Critical", "Emergency", "Shutdown"};
                if (status >= 0 && status < names.length) s.thermal = names[status];
            }
            readStandard(s);
            if (rootEnabled) {
                if (s.elapsed - lastRootRead >= 15000) {
                    try { rootData = RootReader.read(); lastRootRead = s.elapsed; }
                    catch (Exception failure) { rootEnabled = false; rootData = ""; rootError = context.getString(R.string.root_lost); }
                }
                if (rootEnabled) parseRoot(s, rootData);
            }
        } catch (RuntimeException failure) { s.error = context.getString(R.string.read_error); }
        s.root = rootEnabled; s.rootBusy = rootBusy;
        if (!rootError.isEmpty()) s.error = rootError;
        latest = s;
        main.post(() -> { for (Listener listener : listeners) listener.onSnapshot(s); });
    }
    private void readStandard(Snapshot s) {
        File[] zones = new File("/sys/class/thermal").listFiles((dir, name) -> name.matches("thermal_zone[0-9]+"));
        if (zones != null) for (int i = 0; i < Math.min(zones.length, 64); i++) {
            String type = read(new File(zones[i], "type"));
            acceptTemperature(s, type, read(new File(zones[i], "temp")));
        }
        List<String> frequencies = new ArrayList<>();
        File[] policies = new File("/sys/devices/system/cpu/cpufreq").listFiles((dir, name) -> name.matches("policy[0-9]+"));
        if (policies != null) for (int i = 0; i < Math.min(policies.length, 16); i++) {
            String value = frequency(policies[i].getName(), read(new File(policies[i], "scaling_cur_freq")));
            if (!value.isEmpty()) frequencies.add(value);
        }
        if (!frequencies.isEmpty()) s.frequencies = String.join("\n", frequencies);
        String profile = read(new File("/sys/class/thermal/thermal_message/sconfig"));
        if (profile.matches("[0-9]{1,4}")) s.profile = Measurements.thermalProfile(profile);
        s.usbVoltage = Measurements.positiveMicro(read(new File("/sys/class/power_supply/usb/voltage_now")), 50);
        s.usbCurrent = Measurements.positiveMicro(read(new File("/sys/class/power_supply/usb/current_now")), 30);
        s.gpuFrequency = gpu(read(new File("/sys/kernel/ged/hal/current_freqency")));
    }
    private static void parseRoot(Snapshot s, String data) {
        List<String> frequencies = new ArrayList<>();
        for (String line : data.split("\n")) {
            String[] parts = line.split("\\|", -1);
            if (parts.length == 3 && parts[0].equals("T")) acceptTemperature(s, parts[1], parts[2]);
            if (parts.length == 3 && parts[0].equals("F")) {
                String value = frequency(parts[1], parts[2]); if (!value.isEmpty()) frequencies.add(value);
            }
            if (parts.length != 2) continue;
            if (parts[0].equals("P") && parts[1].matches("[0-9]{1,4}")) s.profile = Measurements.thermalProfile(parts[1]);
            if (parts[0].equals("V")) s.usbVoltage = Measurements.positiveMicro(parts[1], 50);
            if (parts[0].equals("I")) s.usbCurrent = Measurements.positiveMicro(parts[1], 30);
            if (parts[0].equals("G")) s.gpuFrequency = gpu(parts[1]);
        }
        if (!frequencies.isEmpty()) s.frequencies = String.join("\n", frequencies);
    }
    private static String read(File file) {
        try (BufferedReader reader = new BufferedReader(new FileReader(file))) {
            char[] buffer = new char[160]; int count = reader.read(buffer);
            return count > 0 ? new String(buffer, 0, count).trim() : "";
        } catch (Exception ignored) { return ""; }
    }
    private static void acceptTemperature(Snapshot s, String type, String raw) {
        // Duchamp maps CPU/GPU/NPU to soc_max. Never relabel battery or skin as CPU.
        String name = type.toLowerCase(Locale.ROOT);
        if (!(name.equals("soc_max") || name.equals("cpu") || name.matches("cpu[0-9]+") || name.equals("cpu-thermal"))) return;
        double value = Measurements.milliCelsius(raw);
        if (Double.isFinite(value) && (!Double.isFinite(s.cpuTemperature) || value > s.cpuTemperature)) {
            s.cpuTemperature = value; s.cpuSource = type;
        }
    }
    private static String frequency(String policy, String raw) {
        try {
            long khz = Long.parseLong(raw.trim());
            return khz > 0 && khz < 10000000 ? policy + "  ·  " + khz / 1000 + " MHz" : "";
        } catch (RuntimeException ignored) { return ""; }
    }
    private static String gpu(String raw) {
        // GED reports OPP index followed by frequency in kHz; reject unfamiliar formats.
        if (!raw.matches("[0-9]+\\s+[0-9]+")) return "Unavailable";
        try {
            long khz = Long.parseLong(raw.split("\\s+")[1]);
            return khz > 0 && khz < 5000000 ? khz / 1000 + " MHz" : "Unavailable";
        } catch (RuntimeException ignored) { return "Unavailable"; }
    }
}
