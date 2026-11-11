package dev.butterd.module;

import android.content.AttributionSource;
import android.os.Binder;
import android.os.Bundle;
import android.os.IBinder;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.Map;

/** HyperOS priority-list protocol. Each operation merges against the current ROM list. */
public final class PowerKeeperRefresh {
    public static final String SERVICE = "powerkeeper";
    public static final String DESCRIPTOR = "com.miui.powerkeeper.priorityList";
    private static final String AUTHORITY = "com.miui.powerkeeper.configure";

    private static String command(String... args) throws Exception {
        return command(true, args);
    }

    private static String command(boolean required, String... args) throws Exception {
        Process process = new ProcessBuilder(args).redirectErrorStream(true).start();
        ByteArrayOutputStream output = new ByteArrayOutputStream();
        try (InputStream input = process.getInputStream()) {
            byte[] buffer = new byte[4096];
            int count;
            while ((count = input.read(buffer)) != -1) {
                if (output.size() + count > 196608) throw new IllegalStateException("PowerKeeper command output too large");
                output.write(buffer, 0, count);
            }
            if (process.waitFor() != 0) {
                if (!required) return "";
                throw new IllegalStateException("PowerKeeper command failed");
            }
            return output.toString("UTF-8").trim();
        } finally { process.destroy(); }
    }

    public static LinkedHashMap<String, Integer> parse(String raw) {
        LinkedHashMap<String, Integer> entries = new LinkedHashMap<>();
        if (raw == null || raw.isEmpty()) return entries;
        if (raw.length() > 131072) throw new IllegalArgumentException("PowerKeeper list too large");
        for (String entry : raw.split(",", -1)) {
            String[] parts = entry.split(":", -1);
            if (parts.length != 2 || !parts[0].matches("[a-zA-Z0-9_]+(\\.[a-zA-Z0-9_]+)+") || parts[0].length() > 180)
                throw new IllegalArgumentException("Invalid PowerKeeper priority entry");
            int hz = Integer.parseInt(parts[1]);
            if (hz < 30 || hz > 240 || entries.put(parts[0], hz) != null)
                throw new IllegalArgumentException("Invalid or duplicate PowerKeeper priority rate");
        }
        return entries;
    }

    private static LinkedHashMap<String, Integer> read() throws Exception {
        Object manager = Class.forName("android.app.ActivityManager").getMethod("getService").invoke(null);
        Class<?> contract = Class.forName("android.app.IActivityManager");
        IBinder token = new Binder();
        Object holder = contract.getMethod("getContentProviderExternal", String.class, int.class, IBinder.class, String.class)
                .invoke(manager, AUTHORITY, 0, token, "butterd-refresh");
        if (holder == null) throw new IllegalStateException("PowerKeeper provider unavailable");
        try {
            Object provider = holder.getClass().getField("provider").get(holder);
            AttributionSource source = new AttributionSource.Builder(android.os.Process.myUid()).setPackageName("com.android.shell").build();
            Bundle result = (Bundle) Class.forName("android.content.IContentProvider")
                    .getMethod("call", AttributionSource.class, String.class, String.class, String.class, Bundle.class)
                    .invoke(provider, source, AUTHORITY, "GET_misc", "key_priv_names", null);
            if (result == null || !result.containsKey("value")) throw new IllegalStateException("PowerKeeper priority read unavailable");
            return parse(result.getString("value"));
        } finally {
            contract.getMethod("removeContentProviderExternalAsUser", String.class, IBinder.class, int.class)
                    .invoke(manager, AUTHORITY, token, 0);
        }
    }

    public static boolean available(int user) throws Exception {
        // PowerKeeper's registered receiver and priority list are owned by user 0.
        if (user != 0) return false;
        if (!command(false, "/system/bin/pm", "path", "--user", "0", "com.miui.powerkeeper").startsWith("package:")) return false;
        read();
        return true;
    }

    public static int run(int user, String app, Integer target) throws Exception {
        if (user != 0) throw new IllegalArgumentException("PowerKeeper refresh supports the owner user only");
        LinkedHashMap<String, Integer> entries = read();
        if (target == null) return entries.containsKey(app) ? entries.get(app) : -1;
        if (target < 0) entries.remove(app); else entries.put(app, target);
        ArrayList<String> list = new ArrayList<>();
        for (Map.Entry<String, Integer> entry : entries.entrySet()) list.add(entry.getKey() + ":" + entry.getValue());
        command("/system/bin/am", "broadcast", "--user", "0", "-a", "com.xiaomi.joyose.HIGH_FPS_LIST",
                "-p", "com.miui.powerkeeper", "--receiver-include-background", "--esal", "high_fps_list", String.join(",", list));
        // The receiver commits the list after rebuilding its in-memory map.
        for (int attempt = 0; attempt < 20; attempt++) {
            if (read().equals(entries)) {
                if (target > 0) {
                    command("/system/bin/am", "broadcast", "--user", "0", "-a", "com.xiaomi.joyose.OVERRIDE_GAME_FRESHRATE",
                            "-p", "com.miui.powerkeeper", "--receiver-include-background", "--es", "override_pkg_name", app,
                            "--ei", "override_freshrate", target.toString());
                }
                return target;
            }
            Thread.sleep(50);
        }
        throw new IllegalStateException("PowerKeeper priority update was not acknowledged");
    }
}
