package dev.butterd.core.ui;

import android.animation.ValueAnimator;
import android.transition.AutoTransition;
import android.transition.TransitionManager;
import android.view.View;
import android.view.ViewGroup;
import com.google.android.material.button.MaterialButton;
import androidx.core.view.ViewCompat;
import dev.butterd.R;

/** Technical information stays available without competing with the primary task. */
public final class Disclosure {
    private Disclosure() { }
    public static void bind(View root, int buttonId, int contentId) {
        MaterialButton button = root.findViewById(buttonId);
        View content = root.findViewById(contentId);
        CharSequence title = button.getText();
        boolean initiallyExpanded = content.getVisibility() == View.VISIBLE;
        if (initiallyExpanded) button.setText(R.string.hide_details);
        ViewCompat.setStateDescription(button, root.getContext().getString(initiallyExpanded ? R.string.details_expanded : R.string.details_collapsed));
        button.setOnClickListener(view -> {
            boolean expanding = content.getVisibility() != View.VISIBLE;
            if (ValueAnimator.areAnimatorsEnabled())
                TransitionManager.beginDelayedTransition((ViewGroup) content.getParent(), new AutoTransition().setDuration(180));
            content.setVisibility(expanding ? View.VISIBLE : View.GONE);
            button.setText(expanding ? root.getContext().getString(R.string.hide_details) : title);
            ViewCompat.setStateDescription(button, root.getContext().getString(expanding ? R.string.details_expanded : R.string.details_collapsed));
        });
    }
}
