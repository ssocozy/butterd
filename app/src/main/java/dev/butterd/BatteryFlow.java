package dev.butterd;

/** Calibrate a vendor current sign only when unplugged flow has an unambiguous direction. */
final class BatteryFlow {
    private int polarity;
    double amperes(int microamps, boolean plugged) {
        if (microamps == Integer.MIN_VALUE) return Double.NaN;
        if (!plugged && microamps != 0) polarity = microamps > 0 ? -1 : 1;
        return microamps / 1_000_000d * (polarity == 0 ? 1 : polarity);
    }
    boolean calibrated() { return polarity != 0; }
}
