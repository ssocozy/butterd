package dev.butterd.core.ui;

import android.content.Context;
import android.view.View;
import android.widget.LinearLayout;
import android.widget.RadioGroup;
import android.widget.ScrollView;
import android.widget.TextView;
import androidx.appcompat.app.AlertDialog;
import androidx.core.content.ContextCompat;
import androidx.core.view.ViewCompat;
import com.google.android.material.dialog.MaterialAlertDialogBuilder;
import com.google.android.material.radiobutton.MaterialRadioButton;
import dev.butterd.R;
import java.util.function.IntConsumer;

/** Browse freely; only the explicit Apply action schedules a controller operation. */
public final class ChoiceDialog {
    private ChoiceDialog() { }
    public static void show(Context context, String title, String[] choices, String[] descriptions,
                            int current, IntConsumer apply) {
        if (choices.length == 0 || descriptions.length != choices.length) return;
        int initial = current >= 0 && current < choices.length ? current : -1;
        int[] selected = {initial};
        float density = context.getResources().getDisplayMetrics().density;
        ScrollView scroll = new ScrollView(context) {
            @Override protected void onMeasure(int width, int height) {
                int maximum = Math.round(getResources().getConfiguration().screenHeightDp
                        * getResources().getDisplayMetrics().density * 0.55f);
                if (View.MeasureSpec.getMode(height) != View.MeasureSpec.UNSPECIFIED)
                    maximum = Math.min(maximum, View.MeasureSpec.getSize(height));
                super.onMeasure(width, View.MeasureSpec.makeMeasureSpec(maximum, View.MeasureSpec.AT_MOST));
            }
        };
        scroll.setVerticalScrollBarEnabled(false);
        LinearLayout body = new LinearLayout(context); body.setOrientation(LinearLayout.VERTICAL);
        body.setPadding(Math.round(24 * density), 0, Math.round(24 * density), Math.round(16 * density));
        scroll.addView(body);
        RadioGroup group = new RadioGroup(context); body.addView(group);
        for (int i = 0; i < choices.length; i++) {
            MaterialRadioButton radio = new MaterialRadioButton(context);
            radio.setId(View.generateViewId()); radio.setText(choices[i]); radio.setTextSize(16);
            radio.setMinHeight(Math.round(52 * density));
            radio.setPadding(0, Math.round(8 * density), 0, Math.round(8 * density));
            radio.setTextColor(ContextCompat.getColor(context, R.color.ink));
            group.addView(radio, new RadioGroup.LayoutParams(-1, -2));
            if (i == initial) group.check(radio.getId());
        }
        TextView preview = new TextView(context);
        preview.setPadding(0, Math.round(16 * density), 0, 0);
        preview.setTextSize(14); preview.setLineSpacing(4 * density, 1f);
        preview.setTextColor(ContextCompat.getColor(context, R.color.secondary));
        preview.setText(initial >= 0 ? descriptions[initial] : context.getString(R.string.choice_preview));
        ViewCompat.setAccessibilityLiveRegion(preview, ViewCompat.ACCESSIBILITY_LIVE_REGION_POLITE);
        body.addView(preview);
        AlertDialog dialog = new MaterialAlertDialogBuilder(context).setTitle(title)
                .setView(scroll).setNegativeButton(android.R.string.cancel, null)
                .setPositiveButton(R.string.apply_choice, (picker, which) -> {
                    if (selected[0] >= 0 && selected[0] != initial) apply.accept(selected[0]);
                }).create();
        dialog.setOnShowListener(ignored -> {
            dialog.getButton(AlertDialog.BUTTON_POSITIVE).setEnabled(false);
            group.setOnCheckedChangeListener((radios, checkedId) -> {
                int index = radios.indexOfChild(radios.findViewById(checkedId));
                if (index < 0 || index >= choices.length) return;
                selected[0] = index; preview.setText(descriptions[index]);
                dialog.getButton(AlertDialog.BUTTON_POSITIVE).setEnabled(index != initial);
            });
        });
        dialog.show();
    }
}
