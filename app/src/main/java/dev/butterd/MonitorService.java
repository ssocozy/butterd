package dev.butterd;

import android.Manifest;
import android.app.Notification;
import android.app.NotificationChannel;
import android.app.NotificationManager;
import android.app.PendingIntent;
import android.app.Service;
import android.content.Intent;
import android.content.pm.PackageManager;
import android.content.pm.ServiceInfo;
import android.os.Build;
import android.os.IBinder;
import android.os.SystemClock;
import androidx.core.app.NotificationCompat;
import androidx.core.app.NotificationManagerCompat;
import androidx.core.content.ContextCompat;
import java.util.Locale;

/** Explicit, user-started monitoring. No boot receiver, wake lock, alarm, or root request. */
public final class MonitorService extends Service implements TelemetryRepository.Listener {
    static final String CHANNEL = "live_monitor", STOP = "dev.butterd.STOP", START_LOG = "dev.butterd.START_LOG";
    private static final int ID = 1;
    private static volatile boolean running;
    private TelemetryRepository repository;
    private long lastUpdate;
    static boolean isRunning() { return running; }
    @Override public void onCreate() {
        super.onCreate();
        NotificationChannel channel = new NotificationChannel(CHANNEL, getString(R.string.monitor_channel), NotificationManager.IMPORTANCE_LOW);
        channel.setDescription(getString(R.string.monitor_channel_description)); channel.setSound(null, null); channel.enableVibration(false);
        getSystemService(NotificationManager.class).createNotificationChannel(channel);
        repository = ((ButterdApp) getApplication()).repository();
    }
    @Override public int onStartCommand(Intent intent, int flags, int startId) {
        if (intent != null && STOP.equals(intent.getAction())) { stopSelf(); return START_NOT_STICKY; }
        try {
            Notification notification = build(getString(R.string.waiting), getString(R.string.notifications_note));
            if (Build.VERSION.SDK_INT >= 34) startForeground(ID, notification, ServiceInfo.FOREGROUND_SERVICE_TYPE_SPECIAL_USE);
            else startForeground(ID, notification);
            // A startForegroundService request must always be acknowledged before stopping,
            // including denied-notification races. Permission denial still prevents sampling.
            if (!notificationsAllowed()) { stopSelf(); return START_NOT_STICKY; }
            running = true; lastUpdate = 0; repository.add(this);
            if (intent != null && START_LOG.equals(intent.getAction())) repository.startRecording();
        } catch (RuntimeException failure) { stopSelf(); }
        return START_NOT_STICKY;
    }
    private boolean notificationsAllowed() {
        if (Build.VERSION.SDK_INT >= 33 && ContextCompat.checkSelfPermission(this, Manifest.permission.POST_NOTIFICATIONS) != PackageManager.PERMISSION_GRANTED) return false;
        NotificationChannel channel = getSystemService(NotificationManager.class).getNotificationChannel(CHANNEL);
        return NotificationManagerCompat.from(this).areNotificationsEnabled() && (channel == null || channel.getImportance() != NotificationManager.IMPORTANCE_NONE);
    }
    @Override public void onSnapshot(Snapshot s) {
        if (!running) return;
        if (!notificationsAllowed()) { stopSelf(); return; }
        long now = SystemClock.elapsedRealtime();
        if (now - lastUpdate < 15000 && lastUpdate != 0) return;
        lastUpdate = now;
        String power = Double.isFinite(s.watts) ? String.format(Locale.getDefault(), "%.1f W", Math.abs(s.watts)) : "Unavailable";
        String direction = Double.isFinite(s.watts) ? (s.watts >= 0 ? "into battery" : "from battery") : "battery power";
        String sleep = Double.isFinite(s.sleepPercent) ? String.format(Locale.getDefault(), "%.0f%%", s.sleepPercent) : "Unavailable";
        String text = (s.recording ? "Recording · " : "") + Measurements.value(s.batteryTemperature, "°C") + " · Deep sleep " + sleep + " · " + Measurements.duration(s.sleepingMillis) + " this session";
        try { getSystemService(NotificationManager.class).notify(ID, build(power + " " + direction, text)); }
        catch (RuntimeException failure) { stopSelf(); }
    }
    private Notification build(String title, String text) {
        PendingIntent open = PendingIntent.getActivity(this, 0, new Intent(this, MainActivity.class), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        PendingIntent stop = PendingIntent.getService(this, 1, new Intent(this, MonitorService.class).setAction(STOP), PendingIntent.FLAG_UPDATE_CURRENT | PendingIntent.FLAG_IMMUTABLE);
        return new NotificationCompat.Builder(this, CHANNEL).setSmallIcon(R.drawable.ic_butterd)
                .setContentTitle(title).setContentText(text).setStyle(new NotificationCompat.BigTextStyle().bigText(text))
                .setContentIntent(open).setOngoing(true).setOnlyAlertOnce(true).setSilent(true)
                .setShowWhen(false).setCategory(NotificationCompat.CATEGORY_STATUS)
                .setForegroundServiceBehavior(NotificationCompat.FOREGROUND_SERVICE_IMMEDIATE)
                .addAction(0, getString(R.string.stop), stop).build();
    }
    @Override public void onDestroy() {
        running = false; if (repository != null) { repository.stopRecording(); repository.remove(this); }
        stopForeground(STOP_FOREGROUND_REMOVE); super.onDestroy();
    }
    @Override public IBinder onBind(Intent intent) { return null; }
}
