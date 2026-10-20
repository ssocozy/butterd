package dev.butterd;

import dev.butterd.feature.cpu.CpuPolicy;
import dev.butterd.feature.cpu.CpuController;
import dev.butterd.feature.kernel.KernelSensors;
import dev.butterd.core.root.RootInventory;

import dev.butterd.feature.display.DisplayController;
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
    private final DiagnosticRecorder recorder;
    private final CpuController cpuController;
    private final DisplayController displayController;
    private final BatteryFlow batteryFlow = new BatteryFlow();
    private volatile boolean recording;
    private String recordingError = "";
    interface Result { void complete(String error); }
    interface Export { void complete(java.io.File file, String error); }

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
            worker.postDelayed(this, recorder.active() || power.isInteractive() ? 3000 : 15000);
        }
    };
    TelemetryRepository(Context context) {
        this.context = context.getApplicationContext();
        recorder = new DiagnosticRecorder(new File(context.getFilesDir(), "diagnostics"), java.util.Map.of(
                "device", Build.DEVICE, "model", Build.MODEL, "android", Build.VERSION.RELEASE,
                "build_id", Build.ID, "security_patch", Build.VERSION.SECURITY_PATCH,
                "kernel", System.getProperty("os.version", "Unknown")));
        cpuController = new CpuController(context);
        displayController = new DisplayController(context);
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
    boolean isRecording() { return recording; }
    boolean hasCpuOriginal(String policy) { return cpuController.hasSaved(policy); }
    void changeDisplay(float hz, boolean adaptive, boolean restore, Result result) {
        worker.post(() -> {
            String error = "";
            if (!rootEnabled) error = "Grant root access inside Butterd first";
            else try { displayController.apply(hz, adaptive, restore); }
            catch (Exception failure) { error = failure.getMessage() == null ? "Display preferences were not verified. Saved originals were retained." : failure.getMessage(); }
            sample(); String message = error; main.post(() -> result.complete(message));
        });
    }
    void changeGovernor(String policy, String governor, boolean restore, Result result) {
        worker.post(() -> {
            String error = "";
            if (!rootEnabled) error = "Grant root access first. No CPU settings were changed.";
            else try {
                cpuController.apply(policy, governor, restore); lastRootRead = 0;
                if (recorder.active()) try {
                    recorder.marker("CPU " + policy + (restore ? " restore requested" : " governor selected: " + governor), SystemClock.elapsedRealtime());
                } catch (Exception recordingFailure) { recordingError = "CPU change verified, but its log marker could not be saved"; }
            } catch (Exception failure) { error = failure.getMessage() == null ? "CPU change could not be verified" : failure.getMessage(); }
            String message = error; sample(); main.post(() -> result.complete(message));
        });
    }
    void startRecording() {
        worker.post(() -> {
            try { recorder.start(SystemClock.elapsedRealtime()); recordingError = ""; }
            catch (Exception failure) { recorder.stop("Recording failed"); recordingError = "Could not create the diagnostic recording"; }
            recording = recorder.active(); lastRootRead = 0; sample();
        });
    }
    void stopRecording() { worker.post(() -> { recorder.stop("Recording saved"); recording = false; if (!listeners.isEmpty()) sample(); }); }
    void marker(String label, Result result) {
        worker.post(() -> {
            String error = "";
            try { recorder.marker(label, SystemClock.elapsedRealtime()); }
            catch (Exception failure) { error = "Could not save the marker. Check whether recording is still active."; }
            sample(); String message = error; main.post(() -> result.complete(message));
        });
    }
    void exportLog(Export result) {
        worker.post(() -> {
            java.io.File file = null; String error = "";
            try { file = recorder.export(new File(context.getCacheDir(), "exports")); }
            catch (Exception failure) { error = failure.getMessage() == null ? "Could not export recording" : failure.getMessage(); }
            java.io.File exported = file; String message = error;
            main.post(() -> result.complete(exported, message));
        });
    }
    boolean isRootEnabled() { return rootEnabled; }
    boolean isRootBusy() { return rootBusy; }
    void requestRoot() {
        if (rootBusy) return;
        rootBusy = true;
        worker.post(() -> {
            try {
                RootInventory.verifyAccess(); rootEnabled = true;
                refreshRoot(SystemClock.elapsedRealtime());
            } catch (Exception failure) {
                rootEnabled = false; rootData = "";
                rootError = failure instanceof java.util.concurrent.TimeoutException
                        ? "Root request timed out. Open your root manager, check Butterd and retry."
                        : failure instanceof java.io.IOException
                        ? "A root shell could not be opened. Check your root installation and retry."
                        : "Root could not be verified. Check Butterd in your root manager and try again.";
            }
            finally { rootBusy = false; sample(); }
        });
    }
    void disableRoot() {
        rootEnabled = false;
        worker.post(() -> { rootData = ""; rootError = ""; lastRootRead = 0; sample(); });
    }
    private void refreshRoot(long elapsed) {
        lastRootRead = elapsed;
        try { rootData = RootInventory.read(); rootError = ""; }
        catch (Exception failure) {
            rootData = "";
            try {
                RootInventory.verifyAccess();
                rootError = "Root connected; extra sensor readings could not be refreshed. Disconnect and reconnect to retry.";
            } catch (Exception accessFailure) {
                rootEnabled = false;
                rootError = "Root access could not be reverified. Check your root manager and reconnect.";
            }
            // Verify permission separately: missing sensors do not establish revocation.
        }
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
                s.cycles = battery.getIntExtra(BatteryManager.EXTRA_CYCLE_COUNT, -1);
                int health = battery.getIntExtra(BatteryManager.EXTRA_HEALTH, BatteryManager.BATTERY_HEALTH_UNKNOWN);
                switch (health) {
                    case BatteryManager.BATTERY_HEALTH_GOOD: s.health = "Good"; break;
                    case BatteryManager.BATTERY_HEALTH_OVERHEAT: s.health = "Overheated"; break;
                    case BatteryManager.BATTERY_HEALTH_DEAD: s.health = "Dead"; break;
                    case BatteryManager.BATTERY_HEALTH_OVER_VOLTAGE: s.health = "Over-voltage"; break;
                    case BatteryManager.BATTERY_HEALTH_UNSPECIFIED_FAILURE: s.health = "Driver reports a fault"; break;
                    case BatteryManager.BATTERY_HEALTH_COLD: s.health = "Cold"; break;
                    default: break;
                }
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
                int microamps = manager.getIntProperty(BatteryManager.BATTERY_PROPERTY_CURRENT_NOW);
                if (microamps != Integer.MIN_VALUE) s.rawCurrentAmps = microamps / 1_000_000d;
                s.currentAmps = batteryFlow.amperes(microamps, s.plugged);
                s.currentSignCalibrated = batteryFlow.calibrated();
                s.watts = Double.isFinite(s.voltage) ? s.currentAmps * s.voltage : Double.NaN;
            }
            if (Build.VERSION.SDK_INT >= 29) {
                int status = ((PowerManager) context.getSystemService(Context.POWER_SERVICE)).getCurrentThermalStatus();
                String[] names = {"None", "Light", "Moderate", "Severe", "Critical", "Emergency", "Shutdown"};
                if (status >= 0 && status < names.length) s.thermal = names[status];
            }
            readStandard(s);
            if (rootEnabled) {
                if (s.elapsed - lastRootRead >= (recorder.active() ? 3000 : 15000)) {
                    refreshRoot(s.elapsed);
                }
                if (rootEnabled) {
                    parseRoot(s, rootData);
                    if (s.chargingSource.equals("Root snapshot")) s.chargingSampleAge = Math.max(0, s.elapsed - lastRootRead);
                }
            }
        } catch (RuntimeException failure) { s.error = context.getString(R.string.read_error); }
        if (s.cycles < 0 && s.kernel.containsKey("cycles")) {
            try { s.cycles = Integer.parseInt(s.kernel.get("cycles")); }
            catch (NumberFormatException ignored) { }
        }
        s.displaySaved = displayController.hasOriginal();
        s.root = rootEnabled; s.rootBusy = rootBusy;
        if (!rootError.isEmpty()) s.error = rootError;
        try { recorder.sample(s); }
        catch (Exception failure) {
            if (failure instanceof DiagnosticRecorder.SizeLimit) recordingError = "";
            else { recorder.stop("Recording stopped after a write error"); recordingError = "Recording stopped: storage could not be written"; }
        }
        recording = recorder.active(); s.recording = recording;
        s.logStatus = recorder.status(); s.recordingError = recordingError;
        latest = s;
        main.post(() -> { for (Listener listener : listeners) listener.onSnapshot(s); });
    }
    private void readStandard(Snapshot s) {
        File[] zones = new File("/sys/class/thermal").listFiles((dir, name) -> name.matches("thermal_zone[0-9]+"));
        if (zones != null) for (int i = 0; i < Math.min(zones.length, 64); i++) {
            String type = read(new File(zones[i], "type"));
            acceptTemperature(s, type, read(new File(zones[i], "temp")));
        }
        android.hardware.display.DisplayManager displays = context.getSystemService(android.hardware.display.DisplayManager.class);
        android.view.Display display = displays.getDisplay(android.view.Display.DEFAULT_DISPLAY);
        if (display != null) {
            s.refreshRate = display.getRefreshRate();
            s.refreshRates = displayController.supportedRates();
            try {
                s.refreshMinimum = android.provider.Settings.System.getString(context.getContentResolver(), "min_refresh_rate");
                s.refreshPeak = android.provider.Settings.System.getString(context.getContentResolver(), "peak_refresh_rate");
            } catch (SecurityException protectedPreference) { /* Display telemetry remains usable. */ }
            java.util.Set<String> modes = new java.util.LinkedHashSet<>();
            for (android.view.Display.Mode mode : display.getSupportedModes())
                modes.add(mode.getPhysicalWidth() + " × " + mode.getPhysicalHeight() + " / " + Measurements.value(mode.getRefreshRate(), "Hz"));
            s.displayModes = String.join("\n", modes);
        }
        for (java.util.Map.Entry<String, String> sensor : KernelSensors.PATHS.entrySet()) {
            String value = KernelSensors.clean(sensor.getKey(), read(new File(sensor.getValue())));
            if (!value.isEmpty()) s.kernel.put(sensor.getKey(), value);
        }
        List<String> frequencies = new ArrayList<>();
        File[] policies = new File("/sys/devices/system/cpu/cpufreq").listFiles((dir, name) -> name.matches("policy[0-9]+"));
        if (policies != null) for (int i = 0; i < Math.min(policies.length, 16); i++) {
            String table = read(new File(policies[i], "scaling_available_frequencies"));
            if (table.matches("[0-9 ]{1,512}")) s.frequencyTables.put(policies[i].getName(), table);
            String value = frequency(policies[i].getName(), read(new File(policies[i], "scaling_cur_freq")));
            if (!value.isEmpty()) frequencies.add(value);
            try { s.policies.add(new CpuPolicy(policies[i].getName(), read(new File(policies[i], "scaling_governor")),
                    read(new File(policies[i], "scaling_available_governors")), read(new File(policies[i], "related_cpus")),
                    read(new File(policies[i], "scaling_min_freq")), read(new File(policies[i], "scaling_max_freq")))); }
            catch (IllegalArgumentException ignored) { }
        }
        if (!frequencies.isEmpty()) s.frequencies = String.join("\n", frequencies);
        String profile = read(new File("/sys/class/thermal/thermal_message/sconfig"));
        if (profile.matches("[0-9]{1,4}")) s.profile = Measurements.thermalProfile(profile);
        s.usbVoltage = Measurements.positiveMicro(read(new File("/sys/class/power_supply/usb/voltage_now")), 50);
        s.usbCurrent = Measurements.positiveMicro(read(new File("/sys/class/power_supply/usb/current_now")), 30);
        String online = read(new File("/sys/devices/system/cpu/online"));
        if (online.matches("[0-9,\\-]{1,64}")) s.onlineCores = online;
        String chargeLimit = read(new File("/sys/class/power_supply/battery/constant_charge_current"));
        s.chargeLimitAmps = Measurements.positiveMicro(chargeLimit, 30);
        s.bypassCommand = bypassState(read(new File("/proc/mtk_battery_cmd/current_cmd")));
        s.powerPath = powerPathState(read(new File("/proc/mtk_battery_cmd/en_power_path")));
        if (Double.isFinite(s.chargeLimitAmps) || !s.bypassCommand.equals("Unavailable") || !s.powerPath.equals("Unavailable")) {
            s.chargingSource = "Standard access"; s.chargingSampleAge = 0;
        }
        s.gpuFrequency = gpu(read(new File("/sys/kernel/ged/hal/current_freqency")));
    }
    private static void parseRoot(Snapshot s, String data) {
        List<String> frequencies = new ArrayList<>();
        List<CpuPolicy> rootPolicies = new ArrayList<>();
        for (String line : data.split("\n")) {
            String[] parts = line.split("\\|", -1);
            if (parts.length == 3 && parts[0].equals("D")) {
                String value = KernelSensors.clean(parts[1], parts[2]);
                if (!value.isEmpty()) s.kernel.put(parts[1], value);
            }
            if (parts.length == 3 && parts[0].equals("A") && CpuPolicy.validId(parts[1]) && parts[2].matches("[0-9 ]{1,512}"))
                s.frequencyTables.put(parts[1], parts[2]);
            if (parts.length == 3 && parts[0].equals("T")) acceptTemperature(s, parts[1], parts[2]);
            if (parts.length == 3 && parts[0].equals("F")) {
                String value = frequency(parts[1], parts[2]); if (!value.isEmpty()) frequencies.add(value);
            }
            if (parts.length == 8 && parts[0].equals("C")) {
                try { rootPolicies.add(new CpuPolicy(parts[1], parts[2], parts[3], parts[4], parts[5], parts[6])); }
                catch (IllegalArgumentException ignored) { }
            }
            if (parts.length != 2) continue;
            if (parts[0].equals("O") && parts[1].matches("[0-9,\\-]{1,64}")) s.onlineCores = parts[1];
            if (parts[0].equals("H")) {
                String state = powerPathState(parts[1]);
                if (!state.equals("Unavailable")) { s.powerPath = state; s.chargingSource = "Root snapshot"; }
            }
            if (parts[0].equals("K")) {
                double limit = Measurements.positiveMicro(parts[1], 30);
                if (Double.isFinite(limit)) { s.chargeLimitAmps = limit; s.chargingSource = "Root snapshot"; }
            }
            if (parts[0].equals("B")) {
                String bypass = bypassState(parts[1]);
                if (!bypass.equals("Unavailable")) { s.bypassCommand = bypass; s.chargingSource = "Root snapshot"; }
            }
            if (parts[0].equals("P") && parts[1].matches("[0-9]{1,4}")) s.profile = Measurements.thermalProfile(parts[1]);
            if (parts[0].equals("V")) { double value = Measurements.positiveMicro(parts[1], 50); if (Double.isFinite(value)) s.usbVoltage = value; }
            if (parts[0].equals("I")) { double value = Measurements.positiveMicro(parts[1], 30); if (Double.isFinite(value)) s.usbCurrent = value; }
            if (parts[0].equals("G")) { String value = gpu(parts[1]); if (!value.equals("Unavailable")) s.gpuFrequency = value; }
        }
        if (!frequencies.isEmpty()) s.frequencies = String.join("\n", frequencies);
        if (!rootPolicies.isEmpty()) s.policies = rootPolicies;
    }
    private static String powerPathState(String raw) {
        return raw.equals("1") ? "Reported enabled" : raw.equals("0") ? "Reported disabled" : "Unavailable";
    }
    private static String bypassState(String raw) {
        if (!raw.trim().matches("[01]\\s+[01]")) return "Unavailable";
        return raw.trim().split("\\s+")[1].equals("1") ? "Charging disable requested" : "Charging disable not requested";
    }
    private static String read(File file) {
        try (BufferedReader reader = new BufferedReader(new FileReader(file))) {
            char[] buffer = new char[1024]; int count = reader.read(buffer);
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
            return khz > 0 && khz < 10000000 ? policy + " / " + khz / 1000 + " MHz" : "";
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
