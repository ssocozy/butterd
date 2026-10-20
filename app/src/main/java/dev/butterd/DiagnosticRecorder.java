package dev.butterd;

import org.json.JSONArray;
import org.json.JSONObject;
import java.io.BufferedWriter;
import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.time.Instant;

/** Worker-thread-only bounded local JSONL recording. No logcat, identifiers, or uploads. */
final class DiagnosticRecorder {
    static final class SizeLimit extends java.io.IOException { SizeLimit() { super("Recording size limit"); } }
    static final long MAX_DURATION_MS = 30 * 60 * 1000L, MAX_BYTES = 2 * 1024 * 1024L;
    private final File directory;
    private final java.util.Map<String, String> metadata;
    private BufferedWriter writer;
    private long started, bytes;
    private int samples;
    private String status = "No recording yet";
    DiagnosticRecorder(File directory) { this(directory, java.util.Collections.emptyMap()); }
    DiagnosticRecorder(File directory, java.util.Map<String, String> metadata) {
        this.directory = directory; this.metadata = new java.util.HashMap<>(metadata);
        if (new File(directory, "session.jsonl").isFile()) status = "Saved recording available";
    }
    boolean active() { return writer != null; }
    String status() { return status; }
    void start(long elapsed) throws Exception {
        if (active()) return;
        if (!directory.isDirectory() && !directory.mkdirs()) throw new IllegalStateException("Could not create recording folder");
        File current = new File(directory, "session.jsonl"), previous = new File(directory, "previous.jsonl");
        if (current.exists()) Files.move(current.toPath(), previous.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
        writer = new BufferedWriter(new OutputStreamWriter(new FileOutputStream(current), StandardCharsets.UTF_8));
        started = elapsed; bytes = 0; samples = 0;
        status = "Recording · 0 samples";
        JSONObject header = new JSONObject().put("type", "session").put("schema", 1).put("app_version", "0.2.0")
                .put("time_utc", Instant.now().toString())
                .put("note", "Battery watts are net battery power, not adapter wattage. Markers are user observations, not verified bypass state.");
        for (java.util.Map.Entry<String, String> entry : metadata.entrySet()) header.put(entry.getKey(), entry.getValue());
        write(header);
    }
    void sample(Snapshot s) throws Exception {
        if (!active()) return;
        if (s.elapsed - started >= MAX_DURATION_MS || bytes >= MAX_BYTES) { stop("Recording stopped at its size or time limit"); return; }
        JSONArray policies = new JSONArray();
        for (CpuPolicy p : s.policies) policies.put(new JSONObject().put("policy", p.id).put("governor", p.governor)
                .put("available", new JSONArray(p.governors)).put("cores", p.cores)
                .put("minimum", p.minimum).put("maximum", p.maximum));
        JSONObject data = new JSONObject().put("type", "sample").put("time_utc", Instant.now().toString())
                .put("elapsed_ms", s.elapsed).put("session_ms", s.elapsed - started)
                .put("battery_w", number(s.watts)).put("battery_current_a", number(s.currentAmps)).put("battery_v", number(s.voltage))
                .put("battery_c", number(s.batteryTemperature)).put("battery_percent", s.capacity < 0 ? JSONObject.NULL : s.capacity)
                .put("plugged", s.plugged).put("charging_reported", s.charging)
                .put("usb_input_v", number(s.usbVoltage)).put("usb_input_a", number(s.usbCurrent))
                .put("charge_limit_a", number(s.chargeLimitAmps)).put("bypass_command", s.bypassCommand).put("primary_power_path_reported", s.powerPath)
                .put("charging_source", s.chargingSource).put("charging_sample_age_ms", s.chargingSampleAge < 0 ? JSONObject.NULL : s.chargingSampleAge)
                .put("thermal_status", s.thermal).put("soc_cpu_c", number(s.cpuTemperature)).put("soc_sensor", s.cpuSource)
                .put("rom_profile", s.profile).put("root_enabled", s.root).put("root_error", s.error)
                .put("online_cores", s.onlineCores).put("cpu_policies", policies).put("cpu_frequencies", s.frequencies)
                .put("gpu_frequency", s.gpuFrequency).put("suspend_ms_process_session", s.sleepingMillis)
                .put("suspend_percent_process_session", number(s.sleepPercent));
        write(data); samples++; status = "Recording · " + samples + " samples";
    }
    void marker(String marker, long elapsed) throws Exception {
        if (!active()) throw new IllegalStateException("Start a recording first");
        write(new JSONObject().put("type", "marker").put("time_utc", Instant.now().toString())
                .put("session_ms", elapsed - started).put("observation", marker));
        writer.flush();
    }
    File export(File cacheDirectory) throws Exception {
        if (writer != null) writer.flush();
        File source = new File(directory, "session.jsonl");
        if (!source.isFile() || source.length() == 0) throw new IllegalStateException("No recording to share yet");
        if (!cacheDirectory.isDirectory() && !cacheDirectory.mkdirs()) throw new IllegalStateException("Could not prepare export");
        File[] old = cacheDirectory.listFiles((dir, name) -> name.startsWith("butterd-diagnostics-") && name.endsWith(".jsonl"));
        if (old != null && old.length >= 3) {
            java.util.Arrays.sort(old, java.util.Comparator.comparingLong(File::lastModified));
            for (int i = 0; i <= old.length - 3; i++) Files.deleteIfExists(old[i].toPath());
        }
        File destination = new File(cacheDirectory, "butterd-diagnostics-" + java.util.UUID.randomUUID() + ".jsonl");
        Files.copy(source.toPath(), destination.toPath(), java.nio.file.StandardCopyOption.REPLACE_EXISTING);
        return destination;
    }
    void stop(String reason) {
        if (writer == null) return;
        try { writer.flush(); writer.close(); }
        catch (Exception ignored) { reason = "Recording stopped; its last sample may be incomplete"; }
        writer = null; status = reason;
    }
    private void write(JSONObject object) throws Exception {
        String line = object.toString() + "\n";
        long length = line.getBytes(StandardCharsets.UTF_8).length;
        if (bytes + length > MAX_BYTES) { stop("Recording stopped at its size limit"); throw new SizeLimit(); }
        writer.write(line); writer.flush(); bytes += length;
    }
    private static Object number(double value) { return Double.isFinite(value) ? value : JSONObject.NULL; }
}
