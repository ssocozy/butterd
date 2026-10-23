use crate::{
    model::{
        choose_clock, number, selectable_governor, touch_value, valid_algorithm, valid_governor,
        valid_policy, Feature,
    },
    Result,
};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::Path,
};

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
pub const FCC: &str = "/sys/class/power_supply/battery/constant_charge_current";
pub const PAUSE: &str = "/proc/mtk_battery_cmd/current_cmd";
pub const BYPASS_ACTIVE: &str = "/sys/class/power_supply/battery/bypass_charging_active";
pub fn bypass_node(hardware: &impl Hardware) -> Result<String> {
    if hardware.read("/sys/class/power_supply/battery/bypass_charging_supported")? != "1" || !matches!(hardware.read(BYPASS_ACTIVE)?.as_str(), "0" | "1") {
        return Err("Kernel does not advertise observable battery bypass".into());
    }
    for node in ["/sys/class/power_supply/battery/bypass_charging", "/sys/class/power_supply/battery/bypass_charge"] {
        if hardware.read(node).is_ok_and(|value| matches!(value.as_str(), "0" | "1")) { return Ok(node.into()); }
    }
    Err("No supported battery bypass control exposed".into())
}

pub trait Hardware {
    fn read(&self, path: &str) -> Result<String>;
    fn write(&self, path: &str, value: &str) -> Result<()>;
    fn read_many(&self, paths: &[&str]) -> Result<Vec<String>> {
        paths.iter().map(|path| self.read(path)).collect()
    }
    fn list(&self, path: &str) -> Result<Vec<String>>;
    fn check_write(&self, _path: &str) -> Result<()> {
        Ok(())
    }
}
pub struct SystemHardware;
fn read_stream(reader: &mut impl Read, limit: u64) -> Result<String> {
    let mut bytes = Vec::new();
    let mut chunk = [0u8;4096];
    loop {
        let remaining = limit.saturating_add(1).saturating_sub(bytes.len() as u64).min(chunk.len() as u64) as usize;
        if remaining == 0 { return Err("Read exceeded its limit".into()); }
        let count = match reader.read(&mut chunk[..remaining]) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if count == 0 { break; }
        bytes.extend_from_slice(&chunk[..count]);
        if bytes.len() as u64 > limit { return Err("Read exceeded its limit".into()); }
    }
    Ok(String::from_utf8(bytes)?)
}
pub fn read_bounded(path: &Path, limit: u64) -> Result<String> {
    // Some vendor proc readers stop after any nonzero offset. Avoid read_to_end's small probe.
    read_stream(&mut fs::File::open(path)?,limit)
}
impl Hardware for SystemHardware {
    fn read_many(&self, paths: &[&str]) -> Result<Vec<String>> {
        if !paths.is_empty() && paths.iter().all(|path| crate::audio::preset(path).is_some()) {
            return crate::audio::read_many(paths);
        }
        paths.iter().map(|path| self.read(path)).collect()
    }
    fn check_write(&self, path: &str) -> Result<()> {
        if path == crate::touch::NODE { return self.check_write(HTSR); }
        if crate::audio::preset(path).is_some() { return Ok(()); }
        if crate::display::node(path).is_some() { return Ok(()); }
        OpenOptions::new().write(true).open(path)?;
        Ok(())
    }
    fn read(&self, path: &str) -> Result<String> {
        if path == crate::touch::NODE { return crate::touch::read(); }
        if let Some(value) = crate::audio::read(path) { return value; }
        if let Some(value) = crate::display::read(path) { return value; }
        Ok(read_bounded(Path::new(path), 4096)?.trim().to_string())
    }
    fn write(&self, path: &str, value: &str) -> Result<()> {
        if path == crate::touch::NODE { return crate::touch::write(value); }
        if let Some(result) = crate::audio::write(path,value) { return result; }
        if let Some(result) = crate::display::write(path,value) { return result; }
        // Open an existing node only. Never create a device path or change its permissions.
        OpenOptions::new()
            .write(true)
            .open(path)?
            .write_all(format!("{value}\n").as_bytes())?;
        Ok(())
    }
    fn list(&self, path: &str) -> Result<Vec<String>> {
        let mut entries = Vec::new();
        for entry in fs::read_dir(path)? {
            if entries.len() >= 256 {
                return Err("Too many hardware entries".into());
            }
            entries.push(
                entry?
                    .file_name()
                    .into_string()
                    .map_err(|_| "Invalid hardware path")?,
            );
        }
        entries.sort();
        Ok(entries)
    }
}
pub fn value(hardware: &impl Hardware, path: &str) -> Result<String> {
    if path == crate::vendor::POWERHAL { return crate::vendor::read(hardware); }
    let raw = hardware.read(path)?;
    if path == GOODIX {
        touch_value(&raw)
    } else {
        Ok(raw)
    }
}
pub fn cooling(hardware: &impl Hardware) -> Result<String> {
    for name in hardware.list(THERMAL)? {
        let valid = name
            .strip_prefix("cooling_device")
            .is_some_and(|id| !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()));
        if valid
            && hardware
                .read(&format!("{THERMAL}/{name}/type"))
                .is_ok_and(|kind| kind == "devfreq-13000000.mali")
        {
            return Ok(format!("{THERMAL}/{name}/cur_state"));
        }
    }
    Err("Duchamp Mali cooling device is not exposed".into())
}
pub fn allowed(hardware: &impl Hardware, feature: &Feature, path: &str) -> bool {
    match feature {
        Feature::PhoneMic => crate::audio::preset(path).is_some(),
        Feature::CpuLock => path
            .strip_prefix(&format!("{CPU}/"))
            .and_then(|v| v.strip_suffix("/scaling_min_freq"))
            .is_some_and(valid_policy),
        Feature::GpuLock => path == GPU || path == GPU_ALT,
        Feature::ChargingThermal => path == CHARGE,
        Feature::GpuThermal => cooling(hardware).is_ok_and(|current| current == path),
        Feature::Touch => path == GOODIX || path == HTSR || path == crate::touch::NODE,
        Feature::Tcp => path == TCP,
        Feature::Governors => path.strip_prefix(&format!("{CPU}/")).and_then(|tail| tail.strip_suffix("/scaling_governor")).is_some_and(valid_policy),
        Feature::Governor(policy) => {
            valid_policy(policy) && path == format!("{CPU}/{policy}/scaling_governor")
        }
        Feature::CpuRange(policy) => valid_policy(policy) && ["scaling_min_freq", "scaling_max_freq"].iter().any(|field| path == format!("{CPU}/{policy}/{field}")),
        Feature::GpuRange => [GPU, GPU_ALT].iter().any(|node| path == *node || path == node.replace("min_freq", "max_freq")),
        Feature::Profile => {
            let cpu = path.strip_prefix(&format!("{CPU}/")).and_then(|tail| tail.split_once('/')).is_some_and(|(policy, field)| valid_policy(policy) && matches!(field, "scaling_min_freq" | "scaling_max_freq"));
            cpu || path == crate::vendor::POWERHAL || allowed(hardware, &Feature::GpuRange, path)
        }
        Feature::Refresh(user) => crate::display::node(path).is_some_and(|(id, _, _)| id == *user),
        Feature::ChargingPause => path == PAUSE || path == FCC,
        Feature::Bypass => bypass_node(hardware).is_ok_and(|node| node == path),
    }
}
pub fn validate_path_value(feature: &Feature, path: &str, value: &str) -> Result<()> {
    if path == crate::touch::NODE {
        if *feature != Feature::Touch { return Err("Touch handover belongs to touch control".into()); }
        crate::touch::normalize(value)?; return Ok(());
    }
    if path == FCC {
        if *feature != Feature::ChargingPause || number(value)? > 30_000_000 { return Err("Invalid charge current baseline".into()); }
        return Ok(());
    }
    if let Some((user,table,key)) = crate::display::node(path) {
        if !matches!(feature,Feature::Refresh(id) if *id == user) { return Err("Display preference belongs to another control or user".into()); }
        crate::display::normalize(table,key,value)?; return Ok(());
    }
    if path == crate::vendor::POWERHAL {
        if *feature != Feature::Profile { return Err("Vendor CPU requests belong to the overall profile".into()); }
        crate::vendor::parse(value)?; return Ok(());
    }
    validate_value(feature, value)
}
pub fn validate_value(feature: &Feature, value: &str) -> Result<()> {
    if *feature == Feature::PhoneMic { crate::audio::normalize(value)?; return Ok(()); }
    if matches!(feature, Feature::Refresh(_)) {
        if value == "null" { return Ok(()); }
        let hz: f64 = value.parse()?;
        if !hz.is_finite() || !(0.0..=144.1).contains(&hz) { return Err("Invalid refresh value".into()); }
        return Ok(());
    }
    if *feature == Feature::ChargingPause {
        let fields: Vec<_> = value.split_whitespace().collect();
        if fields.len() != 2 || fields.iter().any(|v| !matches!(*v, "0" | "1")) { return Err("Invalid charging command".into()); }
        return Ok(());
    }
    if *feature == Feature::Tcp {
        if !valid_algorithm(value) {
            return Err("Invalid TCP algorithm".into());
        }
    } else if matches!(feature, Feature::Governor(_) | Feature::Governors) {
        if !valid_governor(value) {
            return Err("Invalid governor value".into());
        }
    } else {
        number(value)?;
        if matches!(feature, Feature::Touch | Feature::ChargingThermal | Feature::Bypass)
            && value != "0"
            && value != "1"
        {
            return Err("Unrecognized mode value".into());
        }
    }
    Ok(())
}
pub fn plan(
    hardware: &impl Hardware,
    feature: &Feature,
    setting: &str,
) -> Result<Vec<(String, String)>> {
    feature.validate()?;
    let mut targets = Vec::new();
    match feature {
        Feature::PhoneMic => targets = crate::audio::plan(hardware)?,
        Feature::CpuLock => {
            for policy in hardware.list(CPU)? {
                if !valid_policy(&policy) {
                    continue;
                }
                let base = format!("{CPU}/{policy}");
                let minimum = number(&hardware.read(&format!("{base}/scaling_min_freq"))?)?;
                let maximum = number(&hardware.read(&format!("{base}/scaling_max_freq"))?)?;
                let clock = choose_clock(
                    &hardware.read(&format!("{base}/scaling_available_frequencies"))?,
                    minimum,
                    maximum,
                )?;
                targets.push((format!("{base}/scaling_min_freq"), clock.to_string()));
            }
        }
        Feature::GpuLock => {
            let path = if hardware.read(GPU).is_ok() {
                GPU
            } else {
                GPU_ALT
            };
            let parent = Path::new(path)
                .parent()
                .ok_or("Invalid GPU path")?
                .to_string_lossy();
            let clock = choose_clock(
                &hardware.read(&format!("{parent}/available_frequencies"))?,
                number(&hardware.read(path)?)?,
                number(&hardware.read(&format!("{parent}/max_freq"))?)?,
            )?;
            targets.push((path.into(), clock.to_string()));
        }
        Feature::ChargingThermal => targets.push((CHARGE.into(), "1".into())),
        Feature::GpuThermal => targets.push((cooling(hardware)?, "0".into())),
        Feature::Touch => match setting {
            "high" if hardware.read(GOODIX).is_err() => {
                // A complete snapshot is mandatory; never fall back around a failed handover.
                if hardware.read(HTSR).is_ok() {
                    targets.push((crate::touch::NODE.into(),crate::touch::target(hardware,setting)?));
                } else { return Err("HTSR control is not exposed".into()); }
            },
            "high" | "240" | "480" => targets.push((
                GOODIX.into(),
                if setting == "240" { "0" } else { "1" }.into(),
            )),
            _ => return Err("Choose 240, 480 or high".into()),
        },
        Feature::Tcp => {
            if !valid_algorithm(setting)
                || !hardware
                    .read(TCP_AVAILABLE)?
                    .split_whitespace()
                    .any(|value| value == setting)
            {
                return Err("TCP algorithm is not exposed by this kernel".into());
            }
            targets.push((TCP.into(), setting.into()));
        }
        Feature::Governors => {
            let mut policies: Vec<_> = hardware.list(CPU)?.into_iter().filter(|policy| valid_policy(policy)).collect();
            policies.sort(); policies.dedup();
            if policies.is_empty() || policies.len() > 16 { return Err("CPU clusters could not be discovered".into()); }
            for policy in policies { targets.extend(plan(hardware,&Feature::Governor(policy),setting)?); }
        }
        Feature::Governor(policy) => {
            if !selectable_governor(setting) {
                return Err("Unsupported governor choice".into());
            }
            let available =
                hardware.read(&format!("{CPU}/{policy}/scaling_available_governors"))?;
            if !available.split_whitespace().any(|item| item == setting) {
                return Err("Governor is not exposed by this policy".into());
            }
            targets.push((format!("{CPU}/{policy}/scaling_governor"), setting.into()));
        }
        Feature::ChargingPause => {
            let current = hardware.read(PAUSE)?;
            validate_value(feature, &current)?;
            let limit=hardware.read(FCC)?;
            validate_path_value(feature,FCC,&limit)?;
            targets.push((FCC.into(), "0".into()));
            targets.push((PAUSE.into(), format!("{} 1", current.split_whitespace().next().ok_or("Missing charging flag")?)));
        }
        Feature::Bypass => targets.push((bypass_node(hardware)?, "1".into())),
        Feature::CpuRange(policy) => {
            targets = crate::presets::range(hardware, &format!("{CPU}/{policy}"), true, setting)?;
        }
        Feature::GpuRange => {
            let node = if hardware.read(GPU).is_ok() { GPU } else { GPU_ALT };
            targets = crate::presets::range(hardware, node.trim_end_matches("/min_freq"), false, setting)?;
        }
        Feature::Profile | Feature::Refresh(_) => return Err("Use the managed profile controller".into()),
    }
    if targets.is_empty() || targets.len() > 16 {
        return Err("Supported controls are not exposed".into());
    }
    for (path, target) in &targets {
        if !allowed(hardware, feature, path) {
            return Err("Control path is not allowed".into());
        }
        hardware.check_write(path)?;
        validate_value(feature, &value(hardware, path)?)?;
        validate_value(feature, target)?;
    }
    Ok(targets)
}

#[cfg(test)]
mod bounded_read_tests {
    use super::*;
    struct OneReadProc { bytes: Vec<u8>, used: bool }
    impl Read for OneReadProc {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            if self.used { return Ok(0); }
            self.used = true;
            let count = buffer.len().min(self.bytes.len());
            buffer[..count].copy_from_slice(&self.bytes[..count]); Ok(count)
        }
    }
    #[test]
    fn long_vendor_proc_reply_is_read_in_one_buffer() {
        let raw = "1400000 1900000 1600000 2200000 1300000 2300000\n";
        assert!(raw.len()>32);
        let mut proc = OneReadProc { bytes:raw.as_bytes().to_vec(),used:false };
        assert_eq!(read_stream(&mut proc,4096).unwrap(),raw);
        assert!(read_stream(&mut std::io::Cursor::new(vec![b'x';4097]),4096).is_err());
    }
}
