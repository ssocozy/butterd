package dev.butterd;

import dev.butterd.feature.cpu.CpuPolicy;

import org.junit.Test;
import static org.junit.Assert.*;

public class CpuPolicyTest {
    @Test public void arbitraryPathsAndShellTextAreRejected() {
        assertFalse(CpuPolicy.validId("../../battery"));
        assertFalse(CpuPolicy.validId("policy0; reboot"));
        assertFalse(CpuPolicy.validGovernor("schedutil'"));
        assertFalse(CpuPolicy.validGovernor("$(id)"));
        assertFalse(CpuPolicy.selectable("performance"));
        assertFalse(CpuPolicy.selectable("userspace"));
    }
    @Test public void offeredChoicesAreBothSupportedAndAllowlisted() {
        CpuPolicy policy = new CpuPolicy("policy4", "schedutil", "schedutil performance powersave userspace", "4 5 6", "400000", "3200000");
        assertEquals(java.util.Arrays.asList("schedutil", "powersave"), policy.selectableGovernors());
        assertEquals("400 MHz", policy.minimum);
        assertEquals("3200 MHz", policy.maximum);
        assertEquals("4 5 6", policy.cores);
    }
    @Test(expected = IllegalArgumentException.class) public void malformedPoliciesCannotBecomeControls() {
        new CpuPolicy("policy0/../policy7", "schedutil", "schedutil", "0", "0", "0");
    }
}
