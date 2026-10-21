use crate::{model::{choose_clock, number, selectable_governor, touch_value, valid_governor, valid_policy, valid_algorithm, Feature}, Result};
use std::{fs::{self, OpenOptions}, io::{Read, Write}, path::Path};

pub const CPU: &str = "/sys/devices/system/cpu/cpufreq";
pub const GOODIX: &str = "/sys/devices/platform/goodix_ts.0/switch_report_rate";
pub const HTSR: &str = "/sys/devices/virtual/touch/touch_dev/bump_sample_rate";
pub const CHARGE: &str = "/sys/class/power_supply/usb/thermal_remove";
pub const GPU: &str = "/sys/class/devfreq/13000000.mali/min_freq";
pub const GPU_ALT: &str = "/sys/class/misc/mali0/device/devfreq/13000000.mali/min_freq";
pub const THERMAL: &str = "/sys/class/thermal";
pub const TCP: &str = "/proc/sys/net/ipv4/tcp_congestion_control";
pub const TCP_AVAILABLE: &str = "/proc/sys/net/ipv4/tcp_available_congestion_control";
pub const BOOT: &str = "/proc/sys/kernel/random/boot_id";

pub trait Hardware {
    fn read(&self, path: &str) -> Result<String>;
    fn write(&self, path: &str, value: &str) -> Result<()>;
    fn list(&self, path: &str) -> Result<Vec<String>>;
    fn check_write(&self, _path: &str) -> Result<()> { Ok(()) }
}
pub struct SystemHardware;
pub fn read_bounded(path: &Path, limit: u64) -> Result<String> {
    let mut bytes = Vec::new();
    fs::File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit { return Err("Read exceeded its limit".into()); }
    Ok(String::from_utf8(bytes)?)
}
impl Hardware for SystemHardware {
    fn check_write(&self, path: &str) -> Result<()> { OpenOptions::new().write(true).open(path)?; Ok(()) }
    fn read(&self, path: &str) -> Result<String> { Ok(read_bounded(Path::new(path), 4096)?.trim().to_string()) }
    fn write(&self, path: &str, value: &str) -> Result<()> {
        // Open an existing node only. Never create a device path or change its permissions.
        OpenOptions::new().write(true).open(path)?.write_all(format!("{value}\n").as_bytes())?;
        Ok(())
    }
    fn list(&self, path: &str) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            if entries.len() >= 256 { return Err("Too many hardware entries".into()); }
            entries.push(entry?.file_name().into_string().map_err(|_| "Invalid hardware path")?);
        }
        entries.sort(); Ok(entries)
    }
}
pub fn value(hardware: &impl Hardware, path: &str) -> Result<String> {
    let raw = hardware.read(path)?;
    if path == GOODIX { touch_value(&raw) } else { Ok(raw) }
}
pub fn cooling(hardware: &impl Hardware) -> Result<String> {
    for name in hardware.list(THERMAL)? {
        let valid = name.strip_prefix("cooling_device").is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
        if valid && hardware.read(&format!("{THERMAL}/{name}/type")).is_ok_and(|kind| kind == "devfreq-13000000.mali") {
            return Ok(format!("{THERMAL}/{name}/cur_state"));
        }
    }
    Err("Duchamp Mali cooling device is not exposed".into())
}
pub fn allowed(hardware: &impl Hardware, feature: &Feature, path: &str) -> bool {
    match feature {
        Feature::CpuLock => path.strip_prefix(&format!("{CPU}/")).and_then(|v| v.strip_suffix("/scaling_min_freq")).is_some_and(valid_policy),
        Feature::GpuLock => path == GPU || path == GPU_ALT,
        Feature::ChargingThermal => path == CHARGE,
        Feature::GpuThermal => cooling(hardware).is_ok_and(|current| current == path),
        Feature::Touch => path == GOODIX || path == HTSR,
        Feature::Tcp => path == TCP,
        Feature::Governor(policy) => valid_policy(policy) && path == format!("{CPU}/{policy}/scaling_governor"),
    }
}
pub fn validate_value(feature: &Feature, value: &str) -> Result<()> {
    if *feature == Feature::Tcp {
        if !valid_algorithm(value) { return Err("Invalid TCP algorithm".into()); }
    } else if matches!(feature, Feature::Governor(_)) {
        if !valid_governor(value) { return Err("Invalid governor value".into()); }
    } else {
        number(value)?;
        if matches!(feature, Feature::Touch | Feature::ChargingThermal) && value != "0" && value != "1" {
            return Err("Unrecognized mode value".into());
        }
    }
    Ok(())
}
pub fn plan(hardware: &impl Hardware, feature: &Feature, setting: &str) -> Result<Vec<(String, String)>> {
    feature.validate()?;
    let mut targets = Vec::new();
    match feature {
        Feature::CpuLock => {
            for policy in hardware.list(CPU)? {
                if !valid_policy(&policy) { continue; }
                let base = format!("{CPU}/{policy}");
                let minimum = number(&hardware.read(&format!("{base}/scaling_min_freq"))?)?;
                let maximum = number(&hardware.read(&format!("{base}/scaling_max_freq"))?)?;
                let clock = choose_clock(&hardware.read(&format!("{base}/scaling_available_frequencies"))?, minimum, maximum)?;
                targets.push((format!("{base}/scaling_min_freq"), clock.to_string()));
            }
        }
        Feature::GpuLock => {
            let path = if hardware.read(GPU).is_ok() { GPU } else { GPU_ALT };
            let parent = Path::new(path).parent().ok_or("Invalid GPU path")?.to_string_lossy();
            let clock = choose_clock(&hardware.read(&format!("{parent}/available_frequencies"))?,
                number(&hardware.read(path)?)?, number(&hardware.read(&format!("{parent}/max_freq"))?)?)?;
            targets.push((path.into(), clock.to_string()));
        }
        Feature::ChargingThermal => targets.push((CHARGE.into(), "1".into())),
        Feature::GpuThermal => targets.push((cooling(hardware)?, "0".into())),
        Feature::Touch => {
            match setting {
                "high" if hardware.read(GOODIX).is_err() => targets.push((HTSR.into(), "1".into())),
                "high" | "240" | "480" => targets.push((GOODIX.into(), if setting == "240" { "0" } else { "1" }.into())),
                _ => return Err("Choose 240, 480 or high".into()),
            }
        }
        Feature::Tcp => {
            if !valid_algorithm(setting) || !hardware.read(TCP_AVAILABLE)?.split_whitespace().any(|value| value == setting) {
                return Err("TCP algorithm is not exposed by this kernel".into());
            }
            targets.push((TCP.into(), setting.into()));
        }
        Feature::Governor(policy) => {
            if !selectable_governor(setting) { return Err("Unsupported governor choice".into()); }
            let available = hardware.read(&format!("{CPU}/{policy}/scaling_available_governors"))?;
            if !available.split_whitespace().any(|item| item == setting) { return Err("Governor is not exposed by this policy".into()); }
            targets.push((format!("{CPU}/{policy}/scaling_governor"), setting.into()));
        }
    }
    if targets.is_empty() || targets.len() > 16 { return Err("Supported controls are not exposed".into()); }
    for (path, target) in &targets {
        if !allowed(hardware, feature, path) { return Err("Control path is not allowed".into()); }
        hardware.check_write(path)?;
        validate_value(feature, &value(hardware, path)?)?;
        validate_value(feature, target)?;
    }
    Ok(targets)
}
