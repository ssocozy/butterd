package dev.butterd;

import dev.butterd.feature.display.RefreshPreference;
import org.junit.Test;
import static org.junit.Assert.*;

public class RefreshPreferenceTest {
    @Test public void originalAbsenceAndExactStringsArePreserved() {
        RefreshPreference p = new RefreshPreference("null", "119.99999");
        assertEquals("null", p.minimum); assertEquals("119.99999", p.peak);
        assertFalse(p.equals(new RefreshPreference("0", "119.99999")));
    }
    @Test public void rejectsShellTextAndNonFiniteSettings() {
        for (String value : new String[]{null, "120; reboot", "$(id)", "-1", "NaN", "Infinity", "1001"})
            assertFalse(RefreshPreference.validStored(value));
    }
    @Test public void refusesInventedRefreshModes() {
        float[] rates = {60, 120};
        assertTrue(RefreshPreference.supported(120, rates));
        assertFalse(RefreshPreference.supported(90, rates));
        assertFalse(RefreshPreference.supported(Float.NaN, rates));
    }
}
