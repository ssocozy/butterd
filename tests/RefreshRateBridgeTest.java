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
        System.out.println("ROM refresh bridge contract fixtures passed");
    }
}
