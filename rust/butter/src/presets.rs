use crate::{hardware::{Hardware, CPU, GPU, GPU_ALT}, journal::Journal, model::{number, valid_policy}, Result};
use serde::{Deserialize, Serialize};
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Mode { #[default] Balanced, Performance, Battery, Boost }
impl Mode {
    pub fn parse(value: &str) -> Result<Self> {
        match value { "balanced" => Ok(Self::Balanced), "performance" => Ok(Self::Performance), "battery" => Ok(Self::Battery), "boost" => Ok(Self::Boost), _ => Err("Choose balanced, performance or battery".into()) }
    }
}
pub fn clocks(table: &str, min: u64, max: u64) -> Result<Vec<u64>> {
    if min > max { return Err("Inverted frequency limits".into()); }
    let mut values = Vec::new();
    for token in table.split_whitespace() {
        let clock = number(token)?;
        if clock > 0 && clock >= min && clock <= max { values.push(clock); }
    }
    values.sort_unstable(); values.dedup();
    if values.is_empty() { return Err("No clocks within current limits".into()); }
    Ok(values)
}
pub fn range(hardware: &impl Hardware, base: &str, cpu: bool, setting: &str) -> Result<Vec<(String, String)>> {
    let (low, high) = setting.split_once(':').ok_or("Use MIN:MAX in the kernel's native units")?;
    let low = number(low)?; let high = number(high)?;
    let prefix = if cpu { "scaling_" } else { "" };
    let min_path = format!("{base}/{prefix}min_freq");
    let max_path = format!("{base}/{prefix}max_freq");
    let available = if cpu { "scaling_available_frequencies" } else { "available_frequencies" };
    let table = clocks(&hardware.read(&format!("{base}/{available}"))?, number(&hardware.read(&min_path)?)?, number(&hardware.read(&max_path)?)?)?;
    if low > high || !table.contains(&low) || !table.contains(&high) { return Err("Choose advertised clocks inside the current limits; restore first to widen them".into()); }
    Ok(vec![(min_path, low.to_string()), (max_path, high.to_string())])
}
pub fn plan(hardware: &impl Hardware, mode: Mode, baseline: Option<&Journal>) -> Result<Vec<(String, String)>> {
    let mut targets = Vec::new();
    let mut bases: Vec<(String, bool)> = hardware.list(CPU)?.into_iter().filter(|policy| valid_policy(policy)).take(16).map(|policy| (format!("{CPU}/{policy}"), true)).collect();
    if hardware.read(GPU).is_ok() { bases.push((GPU.trim_end_matches("/min_freq").into(), false)); }
    else if hardware.read(GPU_ALT).is_ok() { bases.push((GPU_ALT.trim_end_matches("/min_freq").into(), false)); }
    let original = |path: &str| -> Result<String> {
        if let Some(value) = baseline.and_then(|j| j.cpu_limits.get(path)) { return Ok(value.clone()); }
        if let Some(entry) = baseline.and_then(|j| j.entries.iter().find(|entry| entry.path == path)) { Ok(entry.original.clone()) } else { hardware.read(path) }
    };
    for (base, cpu) in bases {
        let prefix = if cpu { "scaling_" } else { "" };
        let min_path = format!("{base}/{prefix}min_freq"); let max_path = format!("{base}/{prefix}max_freq");
        let minimum = number(&original(&min_path)?)?; let maximum = number(&original(&max_path)?)?;
        let available = if cpu { "scaling_available_frequencies" } else { "available_frequencies" };
        // A missing GPU table skips GPU tuning; malformed or inaccessible CPU capabilities fail the entire plan.
        let table = match hardware.read(&format!("{base}/{available}")) { Ok(table) => table, Err(_) if !cpu => continue, Err(error) => return Err(error) };
        let table = clocks(&table, minimum, maximum)?;
        // Conservative starting points, not benchmark-proven efficiency claims.
        let (low, high) = match mode {
            Mode::Battery => (minimum, table[(table.len() - 1) * 3 / 4]),
            Mode::Performance => (table[(table.len() - 1) / 3], maximum),
            Mode::Boost => (table[(table.len() - 1) / 2], maximum),
            Mode::Balanced => (minimum, maximum),
        };
        targets.push((min_path, low.to_string())); targets.push((max_path, high.to_string()));
    }
    if targets.is_empty() { return Err("No profile controls exposed".into()); }
    Ok(crate::vendor::profile_targets(hardware, targets))
}
