package dev.butterd;

import java.io.File;
import java.nio.file.Files;
import java.util.List;
import org.json.JSONObject;
import org.junit.Test;
import static org.junit.Assert.*;

public class DiagnosticRecorderTest {
    @Test public void exportsAreValidJsonAndDoNotInventMissingMeasurements() throws Exception {
        File base = Files.createTempDirectory("butterd-recorder-test").toFile();
        DiagnosticRecorder recorder = new DiagnosticRecorder(new File(base, "private"));
        try {
            recorder.start(1000);
            Snapshot sample = new Snapshot(); sample.elapsed = 2000;
            recorder.sample(sample); recorder.marker("Before changing ROM settings", 2500);
            File exported = recorder.export(new File(base, "exports"));
            List<String> lines = Files.readAllLines(exported.toPath());
            assertEquals(3, lines.size());
            JSONObject data = new JSONObject(lines.get(1));
            assertTrue(data.isNull("battery_w")); assertTrue(data.isNull("charging_sample_age_ms"));
            assertEquals("marker", new JSONObject(lines.get(2)).getString("type"));
            recorder.stop("Saved"); assertFalse(recorder.active());
            recorder.start(3000); recorder.stop("Saved");
            assertTrue(new File(base, "private/previous.jsonl").isFile());
        } finally {
            recorder.stop("Test ended");
            try (java.util.stream.Stream<java.nio.file.Path> paths = Files.walk(base.toPath())) {
                paths.sorted(java.util.Comparator.reverseOrder()).forEach(path -> { try { Files.deleteIfExists(path); } catch (Exception ignored) { } });
            }
        }
    }
    @Test public void oversizedRecordCannotExceedDiskBudget() throws Exception {
        File base = Files.createTempDirectory("butterd-recorder-size").toFile();
        DiagnosticRecorder recorder = new DiagnosticRecorder(base);
        try {
            recorder.start(1000);
            try { recorder.marker("x".repeat((int) DiagnosticRecorder.MAX_BYTES), 2000); fail("Expected size limit"); }
            catch (java.io.IOException expected) { }
            assertFalse(recorder.active());
            assertTrue(new File(base, "session.jsonl").length() <= DiagnosticRecorder.MAX_BYTES);
        } finally {
            recorder.stop("Test ended"); Files.deleteIfExists(new File(base, "session.jsonl").toPath()); Files.deleteIfExists(base.toPath());
        }
    }
    @Test public void recordingStopsAtTimeLimit() throws Exception {
        File base = Files.createTempDirectory("butterd-recorder-time").toFile();
        DiagnosticRecorder recorder = new DiagnosticRecorder(base);
        try {
            recorder.start(1000); Snapshot sample = new Snapshot();
            sample.elapsed = 1000 + DiagnosticRecorder.MAX_DURATION_MS;
            recorder.sample(sample); assertFalse(recorder.active());
        } finally {
            recorder.stop("Test ended");
            Files.deleteIfExists(new File(base, "session.jsonl").toPath()); Files.deleteIfExists(base.toPath());
        }
    }
}
