package dev.butterd;

import org.junit.Test;
import static org.junit.Assert.*;

public class KernelSensorsTest {
    @Test public void missingAndInconsistentCapacityMustNotInventHealthPercent() {
        assertTrue(Double.isNaN(KernelSensors.remainingCapacity(null, "5000000")));
        assertTrue(Double.isNaN(KernelSensors.remainingCapacity("4500000", "0")));
        assertTrue(Double.isNaN(KernelSensors.remainingCapacity("9000000", "5000000")));
        assertEquals(90, KernelSensors.remainingCapacity("4500000", "5000000"), 0);
    }
    @Test public void inventoryAcceptsOnlyKnownKeysAndRecognizableValues() {
        assertEquals("bbr cubic", KernelSensors.clean("tcp_available", "bbr cubic\n"));
        assertEquals("", KernelSensors.clean("tcp", "bbr; reboot"));
        assertEquals("", KernelSensors.clean("htsr", "1200"));
        assertEquals("0", KernelSensors.clean("htsr", "0"));
        assertEquals("", KernelSensors.clean("unknown", "1"));
        assertEquals("", KernelSensors.clean("full", "-1"));
    }
}
