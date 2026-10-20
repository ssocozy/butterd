package dev.butterd.core.root;

import android.os.Looper;
import java.io.ByteArrayOutputStream;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.FutureTask;
import java.util.concurrent.TimeUnit;

/** Internal process transport. Features own command validation; no exported command endpoint. */
public final class RootExecutor {
    private RootExecutor() { }
    public static String run(String script, int timeoutSeconds, int outputLimit) throws Exception {
        if (Looper.myLooper() == Looper.getMainLooper()) throw new IllegalStateException("Root IO must run off the UI thread");
        if (timeoutSeconds < 1 || timeoutSeconds > 30 || outputLimit < 1 || outputLimit > 65536)
            throw new IllegalArgumentException("Invalid process budget");
        Process process = new ProcessBuilder("su", "-c", "[ \"$(id -u)\" = 0 ] || exit 126; " + script)
                .redirectErrorStream(true).start();
        FutureTask<String> capture = new FutureTask<>(() -> {
            try (InputStream input = process.getInputStream(); ByteArrayOutputStream output = new ByteArrayOutputStream()) {
                byte[] buffer = new byte[1024]; int count;
                while ((count = input.read(buffer)) != -1) {
                    if (output.size() + count > outputLimit) throw new IllegalStateException("Root output exceeded its limit");
                    output.write(buffer, 0, count);
                }
                return output.toString(StandardCharsets.UTF_8.name()).trim();
            }
        });
        Thread reader = new Thread(capture, "butterd-root-output"); reader.setDaemon(true); reader.start();
        try {
            if (!process.waitFor(timeoutSeconds, TimeUnit.SECONDS)) throw new IllegalStateException("Root operation timed out");
            String output = capture.get(1, TimeUnit.SECONDS);
            if (process.exitValue() != 0) throw new IllegalStateException("Root operation failed (" + process.exitValue() + ")");
            return output;
        } finally {
            process.destroyForcibly(); capture.cancel(true);
            try { process.getInputStream().close(); } catch (Exception ignored) { }
        }
    }
}
