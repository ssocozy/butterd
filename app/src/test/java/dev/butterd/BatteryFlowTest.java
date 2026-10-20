package dev.butterd;

import org.junit.Test;
import static org.junit.Assert.*;

public class BatteryFlowTest {
    @Test public void reversedVendorSignIsLearnedFromUnpluggedFlow() {
        BatteryFlow flow = new BatteryFlow();
        assertEquals(-1.5, flow.amperes(1_500_000, false), 0);
        assertTrue(flow.calibrated());
        assertEquals(2, flow.amperes(-2_000_000, true), 0);
        // A connected charger can still supply less power than the phone uses.
        assertEquals(-0.5, flow.amperes(500_000, true), 0);
    }
    @Test public void standardConventionIsPreserved() {
        BatteryFlow flow = new BatteryFlow();
        assertEquals(-1, flow.amperes(-1_000_000, false), 0);
        assertEquals(2, flow.amperes(2_000_000, true), 0);
        assertEquals(-0.5, flow.amperes(-500_000, true), 0);
    }
    @Test public void missingOrZeroCurrentCannotCalibrateDirection() {
        BatteryFlow flow = new BatteryFlow();
        assertTrue(Double.isNaN(flow.amperes(Integer.MIN_VALUE, false)));
        assertEquals(0, flow.amperes(0, false), 0);
        assertFalse(flow.calibrated());
        assertEquals(1, flow.amperes(1_000_000, true), 0);
        assertFalse(flow.calibrated());
    }
}
