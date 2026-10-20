package dev.butterd;

final class Snapshot {
    double watts = Double.NaN, voltage = Double.NaN, batteryTemperature = Double.NaN;
    double cpuTemperature = Double.NaN, usbVoltage = Double.NaN, usbCurrent = Double.NaN;
    double sleepPercent = Double.NaN;
    int capacity = -1;
    long elapsed, sleepingMillis;
    boolean plugged, charging, root, rootBusy;
    String thermal = "Unavailable", cpuSource = "", frequencies = "Unavailable";
    String gpuFrequency = "Unavailable", profile = "Unavailable", error = "";
}
