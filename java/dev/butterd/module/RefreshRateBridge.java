package dev.butterd.module;

import android.os.IBinder;
import java.lang.reflect.Method;
import org.json.JSONObject;

/** Narrow Binder glue. Rust owns policy, journaling, validation and restoration. */
public final class RefreshRateBridge {
    private static final String[] SERVICES = {"refresh_rate_ext", "refresh_rate"};
    private static final String SUFFIX = ".display.IRefreshRateManagerService";

    public static boolean validDescriptor(String value) {
        return value != null && value.length() <= 180 && value.matches("[a-zA-Z_][a-zA-Z0-9_]*(\\.[a-zA-Z_][a-zA-Z0-9_]*)+") && value.endsWith(SUFFIX);
    }

    public static void validateContract(Class<?> type) throws Exception {
        if (!type.isInterface() || type.getMethod("getRefreshRateForPackage", String.class).getReturnType() != int.class
                || type.getMethod("setRefreshRateForPackage", String.class, int.class).getReturnType() != void.class
                || type.getMethod("unsetRefreshRateForPackage", String.class).getReturnType() != void.class) {
            throw new IllegalArgumentException("Unsupported refresh manager contract");
        }
    }

    private static int user(String value) {
        if (!value.matches("[0-9]{1,5}")) throw new IllegalArgumentException("Invalid Android user");
        return Integer.parseInt(value);
    }

    private static void currentUser(int requested) throws Exception {
        int actual = (Integer) Class.forName("android.app.ActivityManager").getMethod("getCurrentUser").invoke(null);
        if (actual != requested) throw new IllegalStateException("Refresh service belongs to a different foreground user");
    }

    private static IBinder binder(String service) throws Exception {
        boolean known = false;
        for (String candidate : SERVICES) if (candidate.equals(service)) known = true;
        if (!known) throw new IllegalArgumentException("Unsupported refresh service name");
        return (IBinder) Class.forName("android.os.ServiceManager").getMethod("checkService", String.class).invoke(null, service);
    }

    private static Object connect(IBinder binder, String descriptor) throws Exception {
        if (binder == null || !binder.pingBinder() || !validDescriptor(descriptor) || !descriptor.equals(binder.getInterfaceDescriptor())) {
            throw new IllegalStateException("Refresh service unavailable or changed");
        }
        Class<?> contract = Class.forName(descriptor);
        validateContract(contract);
        Object proxy = Class.forName(descriptor + "$Stub").getMethod("asInterface", IBinder.class).invoke(null, binder);
        if (proxy == null || !contract.isInstance(proxy)) throw new IllegalStateException("Refresh service proxy unavailable");
        return proxy;
    }

    private static int rate(int value) {
        if (value == -1 || value == 0) return -1;
        if (value < 30 || value > 240) throw new IllegalArgumentException("Unrecognised per-app refresh rate");
        return value;
    }

    private static void run(String[] args) throws Exception {
        if (android.os.Process.myUid() != 0) throw new SecurityException("Root required");
        if (args.length == 2 && args[0].equals("probe")) {
            currentUser(user(args[1]));
            for (String service : SERVICES) {
                IBinder binder = binder(service);
                if (binder == null) continue;
                String descriptor = binder.getInterfaceDescriptor();
                if (!validDescriptor(descriptor)) continue;
                // A recognised service with a broken contract is an error, not a silent fallback.
                connect(binder, descriptor);
                System.out.println(new JSONObject().put("available", true).put("service", service).put("descriptor", descriptor));
                return;
            }
            System.out.println(new JSONObject().put("available", false));
            return;
        }
        if ((args.length != 5 && args.length != 6) || !(args[0].equals("read") || args[0].equals("write"))
                || (args[0].equals("read") != (args.length == 5))) throw new IllegalArgumentException("Invalid refresh operation");
        int requestedUser = user(args[1]);
        currentUser(requestedUser);
        String service = args[2], descriptor = args[3], app = args[4];
        if (app.length() > 180 || !app.contains(".") || !app.matches("[a-zA-Z0-9_]+(\\.[a-zA-Z0-9_]+)+")) throw new IllegalArgumentException("Invalid app package");
        Object proxy = connect(binder(service), descriptor);
        Class<?> contract = Class.forName(descriptor);
        Method get = contract.getMethod("getRefreshRateForPackage", String.class);
        if (args[0].equals("write")) {
            int target = rate(Integer.parseInt(args[5]));
            currentUser(requestedUser);
            if (target < 0) contract.getMethod("unsetRefreshRateForPackage", String.class).invoke(proxy, app);
            else contract.getMethod("setRefreshRateForPackage", String.class, int.class).invoke(proxy, app, target);
            if (rate((Integer) get.invoke(proxy, app)) != target) throw new IllegalStateException("ROM app refresh readback rejected");
        }
        System.out.println(rate((Integer) get.invoke(proxy, app)));
    }

    public static void main(String[] args) {
        try { run(args); System.exit(0); }
        catch (Exception error) {
            Throwable cause = error;
            while (cause.getCause() != null) cause = cause.getCause();
            System.err.println(cause.getClass().getSimpleName() + ": " + cause.getMessage());
            System.exit(1);
        }
    }
}
