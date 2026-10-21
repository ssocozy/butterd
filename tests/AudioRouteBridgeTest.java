package dev.butterd.module;
import java.util.List;
public final class AudioRouteBridgeTest {
    public interface Compatible {
        boolean setPreferredDeviceForCapturePreset(int source, String device);
        boolean clearPreferredDevicesForCapturePreset(int source);
        List<String> getPreferredDevicesForCapturePreset(int source);
    }
    public interface WrongReturn {
        int setPreferredDeviceForCapturePreset(int source, String device);
        boolean clearPreferredDevicesForCapturePreset(int source);
        List<String> getPreferredDevicesForCapturePreset(int source);
    }
    public static void main(String[] args) throws Exception {
        AudioRouteBridge.validateContract(Compatible.class,String.class);
        boolean rejected=false;
        try { AudioRouteBridge.validateContract(WrongReturn.class,String.class); } catch(Exception expected) { rejected=true; }
        if (!rejected) throw new AssertionError("Unsupported audio contract accepted");
        rejected=false;
        try { AudioRouteBridge.validateContract(Compatible.class,Integer.class); } catch(Exception expected) { rejected=true; }
        if (!rejected) throw new AssertionError("Wrong device parameter accepted");
        System.out.println("Audio routing contract fixtures passed");
    }
}
