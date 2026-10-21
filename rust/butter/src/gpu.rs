//! Read-only GPU activity adapters. Missing activity never implies an idle GPU.
use crate::hardware::{Hardware, GPU, GPU_ALT};
pub const GED_UTILIZATION: &str = "/sys/kernel/ged/hal/gpu_utilization";

pub fn parse_ged(raw: &str) -> Option<f64> {
    let values: Vec<_> = raw.split_whitespace().collect();
    if values.len() != 3 { return None; }
    let mut counters = [0u32;3];
    for (index,value) in values.into_iter().enumerate() {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) { return None; }
        counters[index] = value.parse().ok()?;
        if counters[index] > 100 { return None; }
    }
    // GED initializes all three outputs to zero when its provider cannot supply counters.
    if counters.iter().all(|value| *value == 0) { return None; }
    // The ABI is loading, blocked, idle. These are sampled independently; do not sum them.
    Some(f64::from(counters[0]))
}

pub fn activity(hardware: &impl Hardware) -> Option<(f64,&'static str)> {
    for node in [GPU,GPU_ALT] {
        let path = format!("{}/load",node.trim_end_matches("/min_freq"));
        if let Some(value) = hardware.read(&path).ok().and_then(|raw| raw.trim().trim_end_matches('%').parse::<f64>().ok()).filter(|value| value.is_finite() && (0.0..=100.0).contains(value)) {
            return Some((value,"devfreq"));
        }
    }
    hardware.read(GED_UTILIZATION).ok().and_then(|raw| parse_ged(&raw)).map(|value| (value,"MediaTek GED"))
}

#[cfg(test)]
mod tests {
    use super::parse_ged;
    #[test]
    fn loading_is_the_first_counter_and_invalid_samples_stay_unknown() {
        assert_eq!(parse_ged("37 2 61\n"),Some(37.0));
        assert_eq!(parse_ged("0 0 100"),Some(0.0));
        assert_eq!(parse_ged("100 0 0"),Some(100.0));
        for raw in ["", "0 0 0", "37", "37%", "1 2", "1 2 3 4", "101 0 0", "1 101 0", "1 0 101", "-1 0 100", "NaN 0 100", "3.5 0 96", "1% 0 99"] { assert_eq!(parse_ged(raw),None,"{raw}"); }
    }
}
