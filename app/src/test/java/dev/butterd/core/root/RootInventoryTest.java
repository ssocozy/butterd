package dev.butterd.core.root;

import static org.junit.Assert.*;
import org.junit.Test;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Comparator;
import java.util.concurrent.TimeUnit;

public final class RootInventoryTest {
    @Test public void optionalMissingSensorsDoNotFailSuccessfulRootInventory() throws Exception {
        Path sandbox = Files.createTempDirectory("butterd-inventory");
        try {
            // Recreate Duchamp's three-policy topology without touching host sysfs or requiring su.
            for (int index : new int[]{0, 4, 7}) {
                Path policy = sandbox.resolve("sys/devices/system/cpu/cpufreq/policy" + index);
                Files.createDirectories(policy);
                Files.write(policy.resolve("scaling_available_frequencies"), "800000 1600000".getBytes(StandardCharsets.UTF_8));
            }
            String script = RootInventory.command()
                    .replace("[ \"$(id -u)\" = 0 ] || exit 1; ", "")
                    .replace("/sys/", sandbox + "/sys/")
                    .replace("/proc/", sandbox + "/proc/");
            Process process = new ProcessBuilder("sh", "-c", script).redirectErrorStream(true).start();
            assertTrue("Inventory must finish", process.waitFor(5, TimeUnit.SECONDS));
            String output = new String(process.getInputStream().readAllBytes(), StandardCharsets.UTF_8);
            assertEquals("Optional reads and a loop below its bound must exit successfully", 0, process.exitValue());
            assertTrue(output.startsWith("ROOT|0\n"));
            for (int index : new int[]{0, 4, 7}) assertTrue(output.contains("A|policy" + index + "|800000 1600000"));
        } finally {
            try (var paths = Files.walk(sandbox)) {
                for (Path path : paths.sorted(Comparator.reverseOrder()).toList()) Files.deleteIfExists(path);
            }
        }
    }
}
