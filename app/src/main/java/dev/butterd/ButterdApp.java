package dev.butterd;

import android.app.Application;
import androidx.appcompat.app.AppCompatDelegate;

public final class ButterdApp extends Application {
    private TelemetryRepository repository;
    @Override public void onCreate() {
        super.onCreate();
        AppCompatDelegate.setDefaultNightMode(getSharedPreferences("ui", MODE_PRIVATE)
                .getInt("theme", AppCompatDelegate.MODE_NIGHT_FOLLOW_SYSTEM));
        repository = new TelemetryRepository(this);
    }
    public TelemetryRepository repository() { return repository; }
}
