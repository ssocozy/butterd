package dev.butterd.module;

import android.content.Context;
import android.media.AudioDeviceInfo;
import android.media.AudioManager;
import android.media.AudioRecordingConfiguration;
import android.os.Looper;
import java.lang.reflect.Method;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** Input preferences only: no audio capture, output selection or policy-file edits. */
public final class AudioRouteBridge {
    public static void validateContract(Class<?> manager, Class<?> attributes) throws Exception {
        if (manager.getMethod("setPreferredDeviceForCapturePreset", int.class, attributes).getReturnType() != boolean.class
                || manager.getMethod("clearPreferredDevicesForCapturePreset", int.class).getReturnType() != boolean.class
                || !List.class.isAssignableFrom(manager.getMethod("getPreferredDevicesForCapturePreset", int.class).getReturnType())) {
            throw new IllegalArgumentException("Unsupported capture routing API");
        }
    }
    private static int preset(String value) {
        if (!value.equals("1") && !value.equals("5") && !value.equals("7")) throw new IllegalArgumentException("Unsupported capture preset");
        return Integer.parseInt(value);
    }
    private static JSONArray preferences(AudioManager manager, int source, Class<?> attributes) throws Exception {
        List<?> list = (List<?>) AudioManager.class.getMethod("getPreferredDevicesForCapturePreset", int.class).invoke(manager, source);
        if (list.size() > 1) throw new IllegalStateException("Multiple preferred input devices are unsupported");
        JSONArray result = new JSONArray();
        for (Object device : list) {
            if ((Integer) attributes.getMethod("getRole").invoke(device) != 1) throw new IllegalStateException("Unexpected output device in capture preferences");
            result.put(new JSONObject().put("type", attributes.getMethod("getType").invoke(device)).put("address", attributes.getMethod("getAddress").invoke(device)));
        }
        return result;
    }
    private static boolean bluetooth(AudioDeviceInfo device) {
        int type = device.getType();
        return type == AudioDeviceInfo.TYPE_BLUETOOTH_A2DP || type == AudioDeviceInfo.TYPE_BLUETOOTH_SCO || type == AudioDeviceInfo.TYPE_BLE_HEADSET;
    }
    private static JSONObject status(AudioManager manager, Class<?> attributes) throws Exception {
        boolean connected = false;
        for (AudioDeviceInfo device : manager.getDevices(AudioManager.GET_DEVICES_OUTPUTS)) connected |= bluetooth(device);
        JSONObject mic = null;
        for (AudioDeviceInfo device : manager.getDevices(AudioManager.GET_DEVICES_INPUTS)) {
            if (device.getType() == AudioDeviceInfo.TYPE_BUILTIN_MIC) {
                mic = new JSONObject().put("type",device.getType()).put("address",device.getAddress()); break;
            }
        }
        // Also verifies that the caller is authorised to query capture preferences.
        for (int source : new int[]{1,5,7}) preferences(manager,source,attributes);
        JSONArray inputs = new JSONArray();
        for (AudioRecordingConfiguration record : manager.getActiveRecordingConfigurations()) {
            AudioDeviceInfo device = record.getAudioDevice();
            if (device != null) inputs.put(new JSONObject().put("source",record.getClientAudioSource()).put("type",device.getType()).put("phoneMic",device.getType()==AudioDeviceInfo.TYPE_BUILTIN_MIC));
        }
        return new JSONObject().put("available",mic != null).put("bluetoothConnected",connected).put("phoneMic",mic == null ? JSONObject.NULL : mic).put("activeInputs",inputs);
    }
    private static void run(String[] args) throws Exception {
        if (android.os.Process.myUid() != 0) throw new SecurityException("Root required");
        if (Looper.getMainLooper() == null) Looper.prepareMainLooper();
        Class<?> thread = Class.forName("android.app.ActivityThread");
        Object activity = thread.getMethod("systemMain").invoke(null);
        Context context = (Context) thread.getMethod("getSystemContext").invoke(activity);
        AudioManager manager = (AudioManager) context.getSystemService(Context.AUDIO_SERVICE);
        Class<?> attributes = Class.forName("android.media.AudioDeviceAttributes");
        validateContract(AudioManager.class,attributes);
        if (args.length == 1 && args[0].equals("status")) { System.out.println(status(manager,attributes)); return; }
        if (args.length == 2 && args[0].equals("read")) { System.out.println(preferences(manager,preset(args[1]),attributes)); return; }
        if (args.length != 3 || !args[0].equals("write") || args[2].length() > 2048) throw new IllegalArgumentException("Invalid capture routing request");
        int source = preset(args[1]);
        JSONArray devices = new JSONArray(args[2]);
        if (devices.length() > 1) throw new IllegalArgumentException("Only one input preference is supported");
        boolean accepted;
        if (devices.length() == 0) accepted = (Boolean) AudioManager.class.getMethod("clearPreferredDevicesForCapturePreset",int.class).invoke(manager,source);
        else {
            JSONObject device = devices.getJSONObject(0);
            int type = device.getInt("type"); String address = device.getString("address");
            if (type < 1 || type > 100 || address.length() > 256 || address.matches("(?s).*[\\p{Cntrl}].*")) throw new IllegalArgumentException("Invalid input device");
            Object input = attributes.getConstructor(int.class,int.class,String.class).newInstance(1,type,address);
            Method set = AudioManager.class.getMethod("setPreferredDeviceForCapturePreset",int.class,attributes);
            accepted = (Boolean) set.invoke(manager,source,input);
        }
        if (!accepted) throw new IllegalStateException("Audio policy rejected the input preference");
        System.out.println(preferences(manager,source,attributes));
    }
    public static void main(String[] args) {
        try { run(args); System.exit(0); }
        catch (Exception error) {
            Throwable cause = error; while (cause.getCause() != null) cause = cause.getCause();
            System.err.println(cause.getClass().getSimpleName()+": "+cause.getMessage()); System.exit(1);
        }
    }
}
