package dev.butterd.module;

/** Host contract checks; no Binder writes or phone/emulator required. */
public final class RefreshRateBridgeTest {
    public interface Compatible {
        int getRefreshRateForPackage(String name);
        void setRefreshRateForPackage(String name, int hz);
        void unsetRefreshRateForPackage(String name);
    }
    public interface WrongReturn {
        String getRefreshRateForPackage(String name);
        void setRefreshRateForPackage(String name, int hz);
        void unsetRefreshRateForPackage(String name);
    }
    public interface WrongArguments {
        int getRefreshRateForPackage(String name);
        void setRefreshRateForPackage(int uid, int hz);
        void unsetRefreshRateForPackage(String name);
    }
    public static void main(String[] args) throws Exception {
        RefreshRateBridge.validateContract(Compatible.class);
        for (Class<?> type : new Class<?>[]{WrongReturn.class, WrongArguments.class, Object.class}) {
            boolean rejected = false;
            try { RefreshRateBridge.validateContract(type); } catch (Exception expected) { rejected = true; }
            if (!rejected) throw new AssertionError("Unsupported contract accepted");
        }
        if (!RefreshRateBridge.validDescriptor("com.voltage.display.IRefreshRateManagerService")) throw new AssertionError("Voltage interface rejected");
        if (!RefreshRateBridge.validDescriptor("com.bliss.display.IRefreshRateManagerService")) throw new AssertionError("Compatible interface rejected");
        for (String descriptor : new String[]{"android.app.IActivityManager", "../../display.IRefreshRateManagerService", "com.voltage.display.IRefreshRateManagerService$Stub"}) {
            if (RefreshRateBridge.validDescriptor(descriptor)) throw new AssertionError("Invalid descriptor accepted");
        }
        if (!PowerKeeperRefresh.parse(null).isEmpty()) throw new AssertionError("Missing priority list must be empty");
        java.util.Map<String, Integer> entries = PowerKeeperRefresh.parse("com.other.game:90,com.mobile.legends:120");
        if (entries.size() != 2 || entries.get("com.other.game") != 90 || entries.get("com.mobile.legends") != 120)
            throw new AssertionError("PowerKeeper priority list parsed incorrectly");
        for (String raw : new String[]{"com.game:120,com.game:60", "com.game:0", "com.game:241", "com.game:NaN", "com.game:120,", "bad:120"}) {
            boolean rejected = false;
            try { PowerKeeperRefresh.parse(raw); } catch (IllegalArgumentException expected) { rejected = true; }
            if (!rejected) throw new AssertionError("Malformed priority list accepted: " + raw);
        }
        System.out.println("ROM refresh bridge contract fixtures passed");
    }
}
