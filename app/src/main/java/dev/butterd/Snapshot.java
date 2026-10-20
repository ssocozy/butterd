package dev.butterd;

final class Snapshot {
    double watts = Double.NaN, voltage = Double.NaN, batteryTemperature = Double.NaN;
    double cpuTemperature = Double.NaN, usbVoltage = Double.NaN, usbCurrent = Double.NaN;
    double sleepPercent = Double.NaN;
    java.util.List<CpuPolicy> policies = new java.util.ArrayList<>();
    String onlineCores = "Unavailable", bypassCommand = "Unavailable", logStatus = "", recordingError = "";
    String chargingSource = "Unavailable";
    double chargeLimitAmps = Double.NaN, currentAmps = Double.NaN;
    String powerPath = "Unavailable";
    long chargingSampleAge = -1;
    boolean recording;
    int capacity = -1;
    long elapsed, sleepingMillis;
    boolean plugged, charging, root, rootBusy;
    String thermal = "Unavailable", cpuSource = "", frequencies = "Unavailable";
    String gpuFrequency = "Unavailable", profile = "Unavailable", error = "";
}
