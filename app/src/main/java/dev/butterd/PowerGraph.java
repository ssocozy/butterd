package dev.butterd;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.Path;
import android.util.AttributeSet;
import android.view.View;
import androidx.core.content.ContextCompat;

/** Fixed-size ring buffer. Redraws only when a new sample arrives, never on a frame loop. */
public final class PowerGraph extends View {
    private final float[] values = new float[60];
    private int count, next;
    private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path path = new Path();
    public PowerGraph(Context context, AttributeSet attrs) { super(context, attrs); }
    public void add(double value) {
        values[next] = Double.isFinite(value) ? (float) Math.abs(value) : Float.NaN;
        next = (next + 1) % values.length; count = Math.min(count + 1, values.length); invalidate();
    }
    @Override protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        float width = getWidth(), height = getHeight() - 10;
        paint.setColor(ContextCompat.getColor(getContext(), R.color.line)); paint.setStrokeWidth(1);
        for (int i = 1; i <= 3; i++) canvas.drawLine(0, height * i / 3, width, height * i / 3, paint);
        if (count < 2) return;
        float max = 5;
        for (float value : values) if (Float.isFinite(value)) max = Math.max(max, value * 1.15f);
        path.reset(); boolean pen = false;
        for (int i = 0; i < count; i++) {
            float value = values[(next - count + i + values.length) % values.length];
            if (!Float.isFinite(value)) { pen = false; continue; }
            float x = width * i / (values.length - 1), y = height - value / max * (height - 8);
            if (pen) path.lineTo(x, y); else path.moveTo(x, y);
            pen = true;
        }
        paint.setColor(ContextCompat.getColor(getContext(), R.color.ink));
        paint.setStrokeWidth(getResources().getDisplayMetrics().density * 2);
        paint.setStyle(Paint.Style.STROKE); paint.setStrokeJoin(Paint.Join.ROUND); canvas.drawPath(path, paint);
        paint.setStyle(Paint.Style.FILL);
    }
}
