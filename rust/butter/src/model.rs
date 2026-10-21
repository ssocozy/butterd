use crate::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Feature {
    PhoneMic,
    CpuLock,
    GpuLock,
    ChargingThermal,
    GpuThermal,
    Touch,
    Tcp,
    Governor(String),
    Governors,
    Profile,
    CpuRange(String),
    GpuRange,
    Refresh(u32),
    ChargingPause,
    Bypass,
}
impl Feature {
    pub fn parse(value: &str) -> Result<Self> {
        Ok(match value {
            "phone-mic" => Self::PhoneMic,
            "governors" => Self::Governors,
            "cpu-lock" => Self::CpuLock,
            "gpu-lock" => Self::GpuLock,
            "charging-thermal" => Self::ChargingThermal,
            "gpu-thermal" => Self::GpuThermal,
            "touch" => Self::Touch,
            "tcp" => Self::Tcp,
            "profile" => Self::Profile,
            "gpu-range" => Self::GpuRange,
            "charging-pause" => Self::ChargingPause,
            "bypass" => Self::Bypass,
            _ => return Err("Unknown control".into()),
        })
    }
    pub fn key(&self) -> String {
        match self {
            Self::PhoneMic => "phone-mic".into(),
            Self::Governors => "governors".into(),
            Self::CpuLock => "cpu-lock".into(),
            Self::GpuLock => "gpu-lock".into(),
            Self::ChargingThermal => "charging-thermal".into(),
            Self::GpuThermal => "gpu-thermal".into(),
            Self::Touch => "touch".into(),
            Self::Tcp => "tcp".into(),
            Self::Governor(policy) => format!("governor-{policy}"),
            Self::Profile => "profile".into(),
            Self::CpuRange(policy) => format!("range-{policy}"),
            Self::GpuRange => "gpu-range".into(),
            Self::Refresh(user) => format!("refresh-{user}"),
            Self::ChargingPause => "charging-pause".into(),
            Self::Bypass => "bypass".into(),
        }
    }
    pub fn validate(&self) -> Result<()> {
        if let Self::Governor(policy) | Self::CpuRange(policy) = self {
            if !valid_policy(policy) {
                return Err("Invalid CPU policy".into());
            }
        }
        if matches!(self, Self::Refresh(user) if *user > 99999) {
            return Err("Invalid Android user".into());
        }
        Ok(())
    }
    pub fn advanced(&self) -> bool {
        matches!(self, Self::CpuLock | Self::GpuLock | Self::ChargingThermal | Self::GpuThermal | Self::ChargingPause | Self::Bypass)
    }
}
pub fn valid_policy(value: &str) -> bool {
    value.strip_prefix("policy").is_some_and(|suffix| {
        !suffix.is_empty() && suffix.len() <= 3 && suffix.bytes().all(|b| b.is_ascii_digit())
    })
}
pub fn valid_governor(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
pub fn selectable_governor(value: &str) -> bool {
    matches!(
        value,
        "schedutil" | "ondemand" | "conservative" | "powersave" | "interactive"
    )
}
pub fn number(value: &str) -> Result<u64> {
    if value.is_empty() || value.len() > 11 || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid numeric value".into());
    }
    Ok(value.parse()?)
}
pub fn touch_value(raw: &str) -> Result<String> {
    let value = raw.trim().to_ascii_lowercase();
    match value.as_str() {
        "0" | "1" => return Ok(value),
        _ => {}
    }
    match (value.contains("240hz"), value.contains("480hz")) {
        (true, false) => Ok("0".into()),
        (false, true) => Ok("1".into()),
        _ => Err("Unrecognized Goodix report-rate format".into()),
    }
}
pub fn choose_clock(table: &str, minimum: u64, maximum: u64) -> Result<u64> {
    if minimum > maximum {
        return Err("Frequency limits are inconsistent".into());
    }
    let mut chosen: Option<u64> = None;
    for token in table.split_whitespace() {
        let value = number(token)?;
        if value > 0 && value >= minimum && value <= maximum {
            chosen = Some(chosen.unwrap_or(0).max(value));
        }
    }
    chosen.ok_or_else(|| "No advertised clock within current limits".into())
}

pub fn valid_algorithm(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}
