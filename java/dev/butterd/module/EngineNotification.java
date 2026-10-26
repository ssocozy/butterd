package dev.butterd.module;

import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.content.Context;
import android.os.IBinder;
import android.os.Looper;
import android.system.Os;

/** One silent notification on a dedicated Butterd channel. */
public final class EngineNotification {
    public static void main(String[] args) throws Exception {
        if(args.length!=1 || args[0].length()>512)throw new IllegalArgumentException("Invalid notification text");
        // Load the helper as root, then use the same Binder identity as cmd notification.
        if(android.os.Process.myUid()!=0)throw new SecurityException("Root required");
        Os.setgid(2000);Os.setuid(2000);
        if(Looper.myLooper()==null)Looper.prepareMainLooper();
        Class<?> thread=Class.forName("android.app.ActivityThread");
        Object activityThread=thread.getMethod("systemMain").invoke(null);
        Context system=(Context)thread.getMethod("getSystemContext").invoke(activityThread);
        Context context=system.createPackageContext("com.android.shell",Context.CONTEXT_IGNORE_SECURITY);
        Notification notification=new Notification.Builder(context,"butterd_engine")
            .setSmallIcon(android.R.drawable.ic_dialog_info)
            .setContentTitle("Butterd").setContentText(args[0])
            .setStyle(new Notification.BigTextStyle().bigText(args[0]))
            .setSilent(true).setOnlyAlertOnce(true).build();
        Class<?> services=Class.forName("android.os.ServiceManager");
        IBinder binder=(IBinder)services.getMethod("getService",String.class).invoke(null,"notification");
        Class<?> api=Class.forName("android.app.INotificationManager");
        Object manager=Class.forName("android.app.INotificationManager$Stub").getMethod("asInterface",IBinder.class).invoke(null,binder);
        NotificationChannel channel=new NotificationChannel("butterd_engine","Butterd",NotificationManager.IMPORTANCE_LOW);
        channel.setSound(null,null);channel.enableVibration(false);
        Class<?> slice=Class.forName("android.content.pm.ParceledListSlice");
        Object channels=slice.getConstructor(java.util.List.class).newInstance(java.util.Collections.singletonList(channel));
        api.getMethod("createNotificationChannels",String.class,slice).invoke(manager,"com.android.shell",channels);
        // Reuse the CLI notification's package, tag, ID and user so upgrades replace it.
        api.getMethod("enqueueNotificationWithTag",String.class,String.class,String.class,int.class,Notification.class,int.class)
            .invoke(manager,"com.android.shell","com.android.shell","butterd-engine",2020,notification,0);
        System.out.println("posted:");
    }
}
