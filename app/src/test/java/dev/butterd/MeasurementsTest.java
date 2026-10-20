package dev.butterd;

import org.junit.Test;
import static org.junit.Assert.*;

public class MeasurementsTest {
    @Test public void powerUsesBatteryUnitsAndPreservesFlowDirection() {
        assertEquals(8.4, Measurements.batteryWatts(2_000_000, 4200), 0.0001);
        assertEquals(-4.2, Measurements.batteryWatts(-1_000_000, 4200), 0.0001);
        assertEquals(0, Measurements.batteryWatts(0, 4200), 0);
    }
    @Test public void displayedPowerKeepsDirectionAndAvoidsNegativeZero() {
        java.util.Locale previous = java.util.Locale.getDefault();
        try {
            java.util.Locale.setDefault(java.util.Locale.US);
            assertEquals("-4.2 W", Measurements.signedPower(-4.2));
            assertEquals("+8.4 W", Measurements.signedPower(8.4));
            assertEquals("0.0 W", Measurements.signedPower(-0.01));
            assertEquals("Unavailable", Measurements.signedPower(Double.NaN));
            assertEquals("Unavailable", Measurements.signedPower(Double.POSITIVE_INFINITY));
        } finally { java.util.Locale.setDefault(previous); }
    }
    @Test public void missingBatteryReadingsStayUnavailable() {
        assertTrue(Double.isNaN(Measurements.batteryWatts(Integer.MIN_VALUE, 4200)));
        assertTrue(Double.isNaN(Measurements.batteryWatts(1000, -1)));
        assertTrue(Double.isNaN(Measurements.batteryWatts(1000, 25000)));
    }
    @Test public void sleepUsesSuspendTimeRatherThanScreenOffTime() {
        assertEquals(75, Measurements.sleepPercent(40000, 10000), 0);
        assertEquals(0, Measurements.sleepPercent(40000, 40000), 0);
        assertEquals(100, Measurements.sleepPercent(40000, 0), 0);
        assertTrue(Double.isNaN(Measurements.sleepPercent(0, 0)));
        assertTrue(Double.isNaN(Measurements.sleepPercent(1000, 2000)));
        assertEquals(0, Measurements.sleepPercent(1000, 1001), 0);
    }
    @Test public void invalidThermalAndUsbDataAreNotGuessed() {
        assertEquals(43.5, Measurements.milliCelsius("43500\n"), 0);
        assertTrue(Double.isNaN(Measurements.milliCelsius("nan")));
        assertTrue(Double.isNaN(Measurements.milliCelsius("999999")));
        assertEquals(9, Measurements.positiveMicro("9000000", 50), 0);
        assertTrue(Double.isNaN(Measurements.positiveMicro("", 50)));
        assertTrue(Double.isNaN(Measurements.positiveMicro("900000000", 50)));
    }
    @Test public void profilesMatchTheInspectedDuchampTree() {
        assertEquals("Balanced", Measurements.thermalProfile("50"));
        assertEquals("Gaming", Measurements.thermalProfile("19"));
        assertEquals("Battery saver", Measurements.thermalProfile("700"));
        assertEquals("Unknown (123)", Measurements.thermalProfile("123"));
    }
}
