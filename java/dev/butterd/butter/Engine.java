package dev.butterd.butter;

import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.nio.channels.FileChannel;
import java.nio.channels.FileLock;
import java.nio.charset.StandardCharsets;
import java.nio.file.*;
import java.util.*;

/** Session controls. Journals precede writes; no polling, hidden HAL memory edits or boot replay. */
public final class Engine {
    private static final Path STATE = Paths.get("/data/adb/butterd");
    private static final String GOODIX = "/sys/devices/platform/goodix_ts.0/switch_report_rate";
    private static final String HTSR = "/sys/devices/virtual/touch/touch_dev/bump_sample_rate";
    private static final String CHARGE = "/sys/class/power_supply/usb/thermal_remove";
    private static final String GPU = "/sys/class/devfreq/13000000.mali/min_freq";
    private static final String GPU_ALT = "/sys/class/misc/mali0/device/devfreq/13000000.mali/min_freq";
    static String read(Path path) throws Exception {
        byte[] bytes = new byte[4096];
        try (java.io.InputStream input = Files.newInputStream(path)) {
            int count = input.read(bytes);
            return count > 0 ? new String(bytes, 0, count, StandardCharsets.UTF_8).trim() : "";
        }
    }
    public static String normalizeTouch(String raw) {
        String value = raw.trim().toLowerCase(Locale.ROOT);
        if (value.equals("0")) return "0";
        if (value.equals("1")) return "1";
        boolean low = value.contains("240hz"), high = value.contains("480hz");
        if (low && !high) return "0";
        if (high && !low) return "1";
        throw new IllegalArgumentException("Unrecognized Goodix report-rate format");
    }
    public static long chooseClock(String table, long minimum, long maximum) {
        long result = -1;
        for (String token : table.trim().split("\\s+")) {
            if (!token.matches("[0-9]{1,11}")) throw new IllegalArgumentException("Unrecognized frequency table");
            long value = Long.parseLong(token);
            if (value >= minimum && value <= maximum && value > result) result = value;
        }
        if (result <= 0) throw new IllegalArgumentException("No supported clock within current limits");
        return result;
    }
    private static String value(String path) throws Exception {
        String raw = read(Paths.get(path));
        return path.equals(GOODIX) ? normalizeTouch(raw) : raw;
    }
    private static String cooling() throws Exception {
        try (DirectoryStream<Path> entries = Files.newDirectoryStream(Paths.get("/sys/class/thermal"), "cooling_device*")) {
            for (Path entry : entries) if (read(entry.resolve("type")).equals("devfreq-13000000.mali")) return entry.resolve("cur_state").toString();
        }
        throw new IllegalStateException("Duchamp Mali cooling device is not exposed");
    }
    private static boolean allowed(String feature, String path) throws Exception {
        switch (feature) {
            case "cpu-lock": return path.matches("/sys/devices/system/cpu/cpufreq/policy[0-9]{1,3}/scaling_min_freq");
            case "gpu-lock": return path.equals(GPU) || path.equals(GPU_ALT);
            case "charging-thermal": return path.equals(CHARGE);
            case "gpu-thermal": return path.equals(cooling());
            case "touch": return path.equals(GOODIX) || path.equals(HTSR);
            default: return false;
        }
    }
    private static LinkedHashMap<String,String> plan(String feature, String setting) throws Exception {
        LinkedHashMap<String,String> values = new LinkedHashMap<>();
        if (feature.equals("cpu-lock")) {
            try (DirectoryStream<Path> policies = Files.newDirectoryStream(Paths.get("/sys/devices/system/cpu/cpufreq"), "policy*")) {
                for (Path policy : policies) {
                    long min = Long.parseLong(read(policy.resolve("scaling_min_freq"))), max = Long.parseLong(read(policy.resolve("scaling_max_freq")));
                    long clock = chooseClock(read(policy.resolve("scaling_available_frequencies")), min, max);
                    values.put(policy.resolve("scaling_min_freq").toString(), Long.toString(clock));
                }
            }
        } else if (feature.equals("gpu-lock")) {
            Path node = Paths.get(Files.isRegularFile(Paths.get(GPU)) ? GPU : GPU_ALT);
            long min = Long.parseLong(read(node)), max = Long.parseLong(read(node.resolveSibling("max_freq")));
            long clock = chooseClock(read(node.resolveSibling("available_frequencies")), min, max);
            values.put(node.toString(), Long.toString(clock));
        } else if (feature.equals("charging-thermal")) values.put(CHARGE,"1");
        else if (feature.equals("gpu-thermal")) values.put(cooling(),"0");
        else if (feature.equals("touch")) {
            if (setting.equals("high") && !Files.exists(Paths.get(GOODIX))) values.put(HTSR,"1");
            else if (setting.equals("240") || setting.equals("480") || setting.equals("high")) values.put(GOODIX, setting.equals("240") ? "0" : "1");
            else throw new IllegalArgumentException("Choose 240, 480 or high");
        }
        if (values.isEmpty() || values.size() > 16) throw new IllegalStateException("Supported controls are not exposed");
        for (String path : values.keySet()) {
            if (!allowed(feature,path) || !Files.isReadable(Paths.get(path)) || !Files.isWritable(Paths.get(path)))
                throw new IllegalStateException("Control is not accessible: "+path);
            String original = value(path);
            if ((feature.equals("touch") || feature.equals("charging-thermal")) && !original.matches("[01]"))
                throw new IllegalStateException("Unrecognized mode value: "+path);
            if (!original.matches("[0-9]{1,11}")) throw new IllegalStateException("Original value cannot be restored: "+path);
        }
        return values;
    }
    private static void save(Path file, Properties data) throws Exception {
        ByteArrayOutputStream bytes = new ByteArrayOutputStream(); data.store(bytes,"Butterd session journal");
        Path temporary = Files.createTempFile(STATE,"journal-",".tmp");
        try {
            Files.write(temporary,bytes.toByteArray());
            try (FileChannel output = FileChannel.open(temporary,StandardOpenOption.WRITE)) { output.force(true); }
            Files.move(temporary,file,StandardCopyOption.REPLACE_EXISTING,StandardCopyOption.ATOMIC_MOVE);
        } finally { Files.deleteIfExists(temporary); }
    }
    static boolean owned(Properties journal, int i, String actual) {
        return actual.equals(journal.getProperty("expected."+i)) ||
                (journal.getProperty("pending."+i,"false").equals("true") && actual.equals(journal.getProperty("before."+i)));
    }
    private static void apply(String feature, String setting, boolean restoring) throws Exception {
        if (!Arrays.asList("cpu-lock","gpu-lock","charging-thermal","gpu-thermal","touch").contains(feature)) throw new IllegalArgumentException("Unknown control");
        Files.createDirectories(STATE);
        try (FileChannel channel = FileChannel.open(STATE.resolve("controls.lock"),StandardOpenOption.CREATE,StandardOpenOption.WRITE);
             FileLock lock = channel.tryLock()) {
            if (lock == null) throw new IllegalStateException("Another operation is running");
            Path file = STATE.resolve(feature+".properties"); Properties journal = new Properties();
            String boot = read(Paths.get("/proc/sys/kernel/random/boot_id"));
            if (!boot.matches("[0-9a-fA-F-]{36}")) throw new IllegalStateException("Boot identity is not available");
            if (Files.exists(file)) {
                if (Files.size(file) > 32768) throw new IllegalStateException("Invalid journal");
                journal.load(new ByteArrayInputStream(Files.readAllBytes(file)));
                if (!boot.equals(journal.getProperty("boot"))) { Files.delete(file); journal.clear(); }
            }
            if (restoring && journal.isEmpty()) { System.out.println("No active session to restore."); return; }
            LinkedHashMap<String,String> targets = restoring ? new LinkedHashMap<>() : plan(feature,setting);
            if (journal.isEmpty()) {
                journal.setProperty("boot",boot); journal.setProperty("count",Integer.toString(targets.size()));
                int i = 0;
                for (String path : targets.keySet()) {
                    String original = value(path);
                    if (!original.matches("[0-9]{1,11}")) throw new IllegalStateException("Original value cannot be restored: "+path);
                    journal.setProperty("path."+i,path); journal.setProperty("original."+i,original); journal.setProperty("expected."+i,original); i++;
                }
            }
            int count = Integer.parseInt(journal.getProperty("count"));
            if (count < 1 || count > 16) throw new IllegalStateException("Invalid journal count");
            // Preflight the complete transaction before changing any node.
            for (int i = 0; i < count; i++) {
                String path = journal.getProperty("path."+i);
                if (!allowed(feature,path) || !owned(journal,i,value(path))) throw new IllegalStateException("Another controller changed this setting; nothing overwritten: "+path);
                if (restoring) targets.put(path,journal.getProperty("original."+i));
                if (!targets.containsKey(path)) throw new IllegalStateException("Capabilities changed; restore the original session first");
                if (!targets.get(path).matches("[0-9]{1,11}")) throw new IllegalStateException("Invalid target");
            }
            if (targets.size() != count) throw new IllegalStateException("Capabilities changed; restore first");
            for (int i = 0; i < count; i++) {
                String path = journal.getProperty("path."+i), target = targets.get(path), before = value(path);
                if (!owned(journal,i,before)) throw new IllegalStateException("External change detected; saved originals retained");
                journal.setProperty("before."+i,before); journal.setProperty("expected."+i,target); journal.setProperty("pending."+i,"true"); save(file,journal);
                Files.write(Paths.get(path),(target+"\n").getBytes(StandardCharsets.UTF_8),StandardOpenOption.WRITE);
                boolean verified = false;
                for (int retry = 0; retry < 5; retry++) { if (value(path).equals(target)) { verified = true; break; } Thread.sleep(20); }
                if (!verified) throw new IllegalStateException("Readback failed; saved originals retained for Restore");
                journal.setProperty("pending."+i,"false"); save(file,journal);
            }
            if (restoring) Files.delete(file);
            System.out.println(restoring ? "Original settings restored." : "Request verified. ROM services may replace it; Butterd does not continuously force it.");
        }
    }
    private static void status() throws Exception {
        org.json.JSONObject result = new org.json.JSONObject();
        String boot = read(Paths.get("/proc/sys/kernel/random/boot_id"));
        for (String feature : Arrays.asList("cpu-lock","gpu-lock","charging-thermal","gpu-thermal","touch")) {
            org.json.JSONObject entry = new org.json.JSONObject();
            try {
                LinkedHashMap<String,String> desired = plan(feature, feature.equals("touch") ? "high" : "");
                entry.put("available", true); entry.put("nodes", desired.size());
                entry.put("state", "Off");
                Path file = STATE.resolve(feature+".properties");
                if (Files.isRegularFile(file) && Files.size(file) <= 32768) {
                    Properties saved = new Properties(); saved.load(new ByteArrayInputStream(Files.readAllBytes(file)));
                    if (boot.equals(saved.getProperty("boot"))) {
                        int count = Integer.parseInt(saved.getProperty("count")); boolean active = count >= 1 && count <= 16;
                        boolean pending = false;
                        for (int i = 0; active && i < count; i++) {
                            String path = saved.getProperty("path."+i);
                            active = allowed(feature,path) && value(path).equals(saved.getProperty("expected."+i));
                            pending |= saved.getProperty("pending."+i,"false").equals("true");
                        }
                        entry.put("state", pending ? "Interrupted; restore needed" : active ? "Request active" : "Changed by another controller");
                    }
                }
                if (feature.equals("touch")) entry.put("driverHz", Files.exists(Paths.get(GOODIX)) ? normalizeTouch(read(Paths.get(GOODIX))).equals("1") ? 480 : 240 : org.json.JSONObject.NULL);
            } catch (Exception unavailable) {
                entry.put("available",false); entry.put("state","Not exposed"); entry.put("reason",String.valueOf(unavailable.getMessage()));
            }
            result.put(feature,entry);
        }
        org.json.JSONArray cpuPolicies = new org.json.JSONArray();
        try (DirectoryStream<Path> policies = Files.newDirectoryStream(Paths.get("/sys/devices/system/cpu/cpufreq"), "policy*")) {
            for (Path policy : policies) try {
                String id = policy.getFileName().toString(), governor = read(policy.resolve("scaling_governor"));
                if (!id.matches("policy[0-9]{1,3}") || !governor.matches("[a-z][a-z0-9_]{0,31}")) continue;
                org.json.JSONObject item = new org.json.JSONObject(); item.put("id",id); item.put("governor",governor);
                item.put("cores",read(policy.resolve("related_cpus")));
                org.json.JSONArray choices = new org.json.JSONArray();
                for (String candidate : read(policy.resolve("scaling_available_governors")).split("\\s+"))
                    if (Arrays.asList("schedutil","ondemand","conservative","powersave","interactive").contains(candidate)) choices.put(candidate);
                item.put("choices",choices);
                Path saved = STATE.resolve(id); boolean original = false;
                if (Files.isRegularFile(saved)) {
                    String[] fields = read(saved).split("\\s+"); original = fields.length == 3 && fields[0].equals(boot);
                }
                item.put("original",original); cpuPolicies.put(item);
            } catch (Exception inaccessible) { /* Other policies remain usable. */ }
        } catch (Exception inaccessible) { /* Kernel need not expose CPU controls. */ }
        result.put("cpuPolicies",cpuPolicies);
        System.out.println(result.toString());
    }
    public static void main(String[] args) {
        try {
            if (android.system.Os.getuid() != 0) throw new IllegalStateException("Root is required");
            if (args.length == 1 && args[0].equals("status")) { status(); return; }
            if (args.length < 2) throw new IllegalArgumentException("Expected command and feature");
            boolean restore = args[0].equals("restore");
            if (!restore && !args[0].equals("enable")) throw new IllegalArgumentException("Unknown action");
            if (!restore && !args[1].equals("touch") && (args.length < 3 || !args[2].equals("--accept-risk")))
                throw new IllegalArgumentException("Advanced controls require --accept-risk");
            apply(args[1],args.length > 2 ? args[2] : "",restore);
        } catch (Exception failure) { System.err.println(failure.getMessage()); System.exit(1); }
    }
}
