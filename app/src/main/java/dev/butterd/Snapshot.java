package dev.butterd;

import dev.butterd.feature.cpu.CpuPolicy;

final class Snapshot {
    double watts = Double.NaN, voltage = Double.NaN, batteryTemperature = Double.NaN;
    double cpuTemperature = Double.NaN, usbVoltage = Double.NaN, usbCurrent = Double.NaN;
    double sleepPercent = Double.NaN;
    java.util.List<CpuPolicy> policies = new java.util.ArrayList<>();
    String onlineCores = "Unavailable", bypassCommand = "Unavailable", logStatus = "", recordingError = "";
    String chargingSource = "Unavailable";
    double chargeLimitAmps = Double.NaN, currentAmps = Double.NaN, rawCurrentAmps = Double.NaN;
    boolean currentSignCalibrated;
    String powerPath = "Unavailable";
    long chargingSampleAge = -1;
    boolean recording;
    int capacity = -1, cycles = -1;
    String health = "", displayModes = "";
    double refreshRate = Double.NaN;
    float[] refreshRates = new float[0];
    String refreshMinimum, refreshPeak;
    boolean displaySaved;
    java.util.Map<String, String> kernel = new java.util.HashMap<>();
    java.util.Map<String, String> frequencyTables = new java.util.HashMap<>();
    long elapsed, sleepingMillis;
    boolean plugged, charging, root, rootBusy;
    String thermal = "Unavailable", cpuSource = "", frequencies = "Unavailable";
    String gpuFrequency = "Unavailable", profile = "Unavailable", error = "";
}
