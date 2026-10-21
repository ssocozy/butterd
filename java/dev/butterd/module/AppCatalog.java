package dev.butterd.module;

import android.content.Context;
import android.content.pm.ApplicationInfo;
import android.content.pm.PackageManager;
import android.graphics.Bitmap;
import android.graphics.Canvas;
import android.graphics.drawable.Drawable;
import android.os.Looper;
import android.os.UserHandle;
import android.util.Base64;
import java.io.ByteArrayOutputStream;
import java.lang.reflect.Constructor;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import org.json.JSONArray;
import org.json.JSONObject;

/** Read-only PackageManager adapter. No policy, settings writes or persistent process. */
public final class AppCatalog {
    public static void main(String[] args) throws Exception {
        if (args.length != 1 || !args[0].matches("[0-9]{1,5}")) throw new IllegalArgumentException("Expected Android user");
        if (Looper.getMainLooper() == null) Looper.prepareMainLooper();
        Class<?> threadClass = Class.forName("android.app.ActivityThread");
        Object thread = threadClass.getMethod("systemMain").invoke(null);
        Context context = (Context) threadClass.getMethod("getSystemContext").invoke(thread);
        Constructor<UserHandle> constructor = UserHandle.class.getDeclaredConstructor(int.class);
        constructor.setAccessible(true);
        UserHandle user = constructor.newInstance(Integer.parseInt(args[0]));
        context = (Context) Context.class.getMethod("createContextAsUser", UserHandle.class, int.class).invoke(context, user, 0);
        PackageManager manager = context.getPackageManager();
        List<JSONObject> rows = new ArrayList<>();
        int iconBudget = 0;
        for (ApplicationInfo info : manager.getInstalledApplications(0)) {
            if (rows.size() >= 512) break;
            if (!info.enabled || manager.getLaunchIntentForPackage(info.packageName) == null) continue;
            JSONObject row = new JSONObject();
            row.put("package", info.packageName);
            CharSequence label = info.loadLabel(manager);
            String name = label == null ? info.packageName : label.toString().replaceAll("[\\p{Cntrl}]", "");
            row.put("label", name.substring(0, Math.min(name.length(), 120)));
            row.put("system", (info.flags & ApplicationInfo.FLAG_SYSTEM) != 0);
            row.put("game", info.category == ApplicationInfo.CATEGORY_GAME);
            if (iconBudget < 262144) {
                Bitmap bitmap = null;
                try {
                    Drawable drawable = info.loadIcon(manager);
                    bitmap = Bitmap.createBitmap(40, 40, Bitmap.Config.ARGB_8888);
                    drawable.setBounds(0, 0, 40, 40); drawable.draw(new Canvas(bitmap));
                    ByteArrayOutputStream stream = new ByteArrayOutputStream();
                    bitmap.compress(Bitmap.CompressFormat.PNG, 100, stream);
                    byte[] png = stream.toByteArray();
                    if (png.length <= 2048) {
                        String icon = "data:image/png;base64," + Base64.encodeToString(png, Base64.NO_WRAP);
                        row.put("icon", icon); iconBudget += icon.length();
                    }
                } catch (Exception ignored) { /* Labels and package names remain usable. */ }
                finally { if (bitmap != null) bitmap.recycle(); }
            }
            rows.add(row);
        }
        rows.sort(Comparator.comparing((JSONObject row) -> row.optString("label"), String.CASE_INSENSITIVE_ORDER));
        JSONArray apps = new JSONArray(); for (JSONObject row : rows) apps.put(row);
        System.out.println(new JSONObject().put("user", Integer.parseInt(args[0])).put("apps", apps).put("source", "PackageManager"));
        System.exit(0);
    }
}
