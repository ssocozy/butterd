package dev.butterd.feature.display;

import android.view.View;
import android.widget.TextView;
import com.google.android.material.button.MaterialButton;
import dev.butterd.core.ui.ChoiceDialog;
import dev.butterd.R;
import java.util.Locale;

/** Display UI owns selection and pending state; the host only schedules its feature command. */
public final class DisplayPanel {
    public interface Result { void complete(String error); }
    public interface Host {
        void change(float hz, boolean adaptive, boolean restore, Result callback);
        void access();
        void feedback(String message);
    }
    private final View root;
    private final Host host;
    private final MaterialButton choose, restore;
    private float[] rates = new float[0];
    private boolean busy, rootGranted, saved;
    private String currentMinimum, currentPeak;
    public DisplayPanel(View root, Host host) {
        this.root = root; this.host = host;
        choose = root.findViewById(R.id.display_choose); restore = root.findViewById(R.id.display_restore);
        choose.setOnClickListener(v -> select());
        restore.setOnClickListener(v -> change(0, false, true));
        root.findViewById(R.id.display_access).setOnClickListener(v -> host.access());
    }
    public void render(float reported, float[] supported, String minimum, String peak, boolean granted, boolean hasOriginal) {
        rates = supported.clone(); rootGranted = granted; saved = hasOriginal;
        currentMinimum = minimum; currentPeak = peak;
        set(R.id.display_rate, Float.isFinite(reported) ? String.format(Locale.getDefault(), "%.0f Hz", reported) : "—");
        set(R.id.display_request, "Minimum " + label(minimum) + "  /  Peak " + label(peak));
        set(R.id.display_control_note, granted ? "Choose a supported rate here. Android may lower the reported rate for power, thermal or app constraints." : "Monitoring works without root. Grant root inside Butterd to change refresh preferences.");
        updateButtons();
    }
    private void updateButtons() {
        choose.setEnabled(rootGranted && !busy && rates.length > 0);
        restore.setEnabled(rootGranted && !busy && saved);
        restore.setVisibility(saved ? View.VISIBLE : View.GONE);
        root.findViewById(R.id.display_access).setVisibility(rootGranted ? View.GONE : View.VISIBLE);
    }
    private void select() {
        if (!rootGranted || busy || rates.length == 0) return;
        float[] offered = rates.clone();
        String[] choices = new String[offered.length + 1];
        choices[0] = String.format(Locale.getDefault(), "Adaptive up to %.0f Hz", offered[offered.length - 1]);
        for (int i = 0; i < offered.length; i++) choices[i + 1] = String.format(Locale.getDefault(), "%.0f Hz preference", offered[i]);
        String[] previews = new String[choices.length];
        int selected = -1;
        for (int i = 0; i < choices.length; i++) {
            float rate = i == 0 ? offered[offered.length - 1] : offered[i - 1];
            previews[i] = String.format(Locale.getDefault(), i == 0
                    ? "Allow Android to vary refresh rate up to %.0f Hz."
                    : "Request a fixed %.0f Hz refresh rate.", rate)
                    + " This global preference can remain after closing Butterd or rebooting. Restore returns the saved originals. Android power and thermal constraints still apply.";
            try {
                float min = Float.parseFloat(currentMinimum), peak = Float.parseFloat(currentPeak);
                if (Math.abs(peak - rate) < 0.1f && (i == 0 ? min == 0 : Math.abs(min - rate) < 0.1f)) selected = i;
            } catch (RuntimeException ignored) { }
        }
        ChoiceDialog.show(root.getContext(), "Refresh preference", choices, previews, selected, item ->
                change(item == 0 ? offered[offered.length - 1] : offered[item - 1], item == 0, false));
    }
    private void change(float rate, boolean adaptive, boolean restoring) {
        busy = true; updateButtons();
        host.change(rate, adaptive, restoring, error -> {
            busy = false;
            if (!root.isAttachedToWindow()) return;
            updateButtons();
            host.feedback(error.isEmpty() ? restoring ? "Original display preferences restored" : "Refresh preference saved and verified" : error);
        });
    }
    private static String label(String value) { return value == null || value.equals("null") ? "default" : value + " Hz"; }
    private void set(int id, String text) { TextView view = root.findViewById(id); if (!text.contentEquals(view.getText())) view.setText(text); }
}
