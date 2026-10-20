package dev.butterd.feature.cpu;

import android.view.LayoutInflater;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.TextView;
import androidx.core.content.ContextCompat;
import com.google.android.material.button.MaterialButton;
import dev.butterd.core.ui.ChoiceDialog;
import dev.butterd.core.ui.Disclosure;
import dev.butterd.R;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/** CPU screen state and interactions stay with the CPU feature. */
public final class CpuPanel {
    public interface Result { void complete(String error); }
    public interface Host {
        boolean hasOriginal(String policy);
        void change(String policy, String governor, boolean restore, Result callback);
        void access();
        void feedback(String message);
    }
    private final View root;
    private final Host host;
    private String signature = "";
    private final java.util.Set<String> expanded = new java.util.HashSet<>();
    private boolean busy, granted, checking;
    private List<CpuPolicy> policies = java.util.Collections.emptyList();
    private Map<String, String> tables = java.util.Collections.emptyMap();
    public CpuPanel(View root, Host host) {
        this.root = root; this.host = host;
        root.findViewById(R.id.cpu_access).setOnClickListener(v -> host.access());
    }
    public void render(List<CpuPolicy> policies, String online, Map<String, String> tables, boolean granted, boolean checking) {
        this.policies = policies; this.tables = tables; this.granted = granted; this.checking = checking;
        ((TextView) root.findViewById(R.id.cpu_online)).setText(display(online));
        ((TextView) root.findViewById(R.id.cpu_control_note)).setText(granted ? R.string.cpu_control_note : R.string.cpu_root_required);
        root.findViewById(R.id.cpu_access).setVisibility(granted ? View.GONE : View.VISIBLE);
        root.findViewById(R.id.cpu_access).setEnabled(!checking);
        rebuild();
    }
    private void rebuild() {
        StringBuilder next = new StringBuilder().append(granted).append(checking).append(busy).append(tables);
        for (CpuPolicy p : policies) next.append(p.id).append(p.governor).append(p.governors).append(p.cores).append(p.minimum).append(p.maximum).append(host.hasOriginal(p.id));
        if (next.toString().equals(signature)) return;
        signature = next.toString();
        LinearLayout list = root.findViewById(R.id.cpu_policies);
        for (int i = 0; i < list.getChildCount(); i++) {
            View old = list.getChildAt(i);
            if (old.getTag() instanceof String) {
                View details = old.findViewById(R.id.policy_details);
                if (details != null && details.getVisibility() == View.VISIBLE) expanded.add((String) old.getTag());
                else expanded.remove((String) old.getTag());
            }
        }
        list.removeAllViews();
        if (policies.isEmpty()) {
            TextView empty = new TextView(root.getContext());
            empty.setText(checking ? R.string.cpu_empty_checking : granted ? R.string.cpu_empty_root : R.string.cpu_empty_standard);
            empty.setTextColor(ContextCompat.getColor(root.getContext(), R.color.secondary)); empty.setPadding(0, dp(20), 0, 0); list.addView(empty);
        }
        for (CpuPolicy p : policies) {
            View row = LayoutInflater.from(root.getContext()).inflate(R.layout.cpu_policy, list, false);
            ((TextView) row.findViewById(R.id.policy_title)).setText("Cores " + display(p.cores));
            ((TextView) row.findViewById(R.id.policy_current)).setText(p.governor);
            ((TextView) row.findViewById(R.id.policy_range)).setText(range(p.minimum) + " – " + range(p.maximum));
            ((TextView) row.findViewById(R.id.policy_details)).setText(p.id + "\nAvailable governors: "
                    + String.join(", ", p.governors) + "\n\nSupported clocks\n" + clocks(tables.get(p.id)));
            row.setTag(p.id);
            boolean wasExpanded = expanded.contains(p.id);
            row.findViewById(R.id.policy_details).setVisibility(wasExpanded ? View.VISIBLE : View.GONE);
            Disclosure.bind(row, R.id.policy_details_toggle, R.id.policy_details);
            MaterialButton choose = row.findViewById(R.id.policy_governor), restore = row.findViewById(R.id.policy_restore);
            choose.setVisibility(granted ? View.VISIBLE : View.GONE); choose.setEnabled(granted && !busy && !p.selectableGovernors().isEmpty());
            restore.setVisibility(granted && host.hasOriginal(p.id) ? View.VISIBLE : View.GONE); restore.setEnabled(granted && !busy);
            choose.setOnClickListener(v -> {
                String[] choices = p.selectableGovernors().toArray(new String[0]);
                String[] previews = new String[choices.length];
                int selected = -1;
                for (int i = 0; i < choices.length; i++) {
                    if (choices[i].equals(p.governor)) selected = i;
                    previews[i] = "Change cores " + display(p.cores) + " from " + p.governor + " to " + choices[i]
                            + ". " + root.getContext().getString(R.string.cpu_confirm)
                            + " The selection can remain after closing Butterd. Restore returns the saved original.";
                }
                ChoiceDialog.show(root.getContext(), "Choose governor", choices, previews, selected,
                        item -> change(p.id, choices[item], false));
            });
            restore.setOnClickListener(v -> change(p.id, "", true)); list.addView(row);
        }
    }
    private void change(String policy, String governor, boolean restore) {
        busy = true; rebuild(); host.feedback(root.getContext().getString(R.string.cpu_pending));
        host.change(policy, governor, restore, error -> {
            busy = false; signature = "";
            if (!root.isAttachedToWindow()) return;
            rebuild(); host.feedback(error.isEmpty() ? root.getContext().getString(restore ? R.string.cpu_restored : R.string.cpu_changed) : error);
        });
    }
    private static String display(String value) { return value == null || value.equals("Unavailable") || value.isEmpty() ? "—" : value; }
    private static String range(String raw) {
        try { return String.format(Locale.getDefault(), "%.2f GHz", Double.parseDouble(raw.replace(" MHz", "")) / 1000d); }
        catch (RuntimeException ignored) { return "—"; }
    }
    private static String clocks(String table) {
        if (table == null || !table.matches("[0-9 ]{1,512}")) return "Not reported";
        java.util.List<String> values = new java.util.ArrayList<>();
        for (String value : table.trim().split(" +")) {
            try { long khz = Long.parseLong(value); if (khz > 0 && khz < 10000000) values.add(String.format(Locale.getDefault(), "%.2f", khz / 1000000d)); }
            catch (NumberFormatException ignored) { }
        }
        return values.isEmpty() ? "Not reported" : String.join(", ", values) + " GHz";
    }
    private int dp(int size) { return Math.round(size * root.getResources().getDisplayMetrics().density); }
}
