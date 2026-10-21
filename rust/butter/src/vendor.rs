//! Readable stock MT6897 PowerHAL QoS ABI: one min/max pair per CPU policy.
use crate::{hardware::{Hardware, CPU}, Result};
pub const POWERHAL: &str = "/proc/powerhal_cpu_ctrl/perfserv_freq";
pub fn parse(text: &str) -> Result<Vec<i64>> {
    let values: Vec<i64> = text.split_whitespace().map(str::parse).collect::<std::result::Result<_,_>>()?;
    if values.len() != 6 { return Err("Unrecognized PowerHAL CPU request layout".into()); }
    for pair in values.chunks_exact(2) {
        if !(pair[0] == -1 || pair[0] == 0 || (100000..=4000000).contains(&pair[0]))
            || !(pair[1] == -1 || (100000..=4000000).contains(&pair[1]))
            || (pair[0] >= 0 && pair[1] >= 0 && pair[0] > pair[1]) { return Err("Invalid PowerHAL CPU request".into()); }
    }
    Ok(values)
}
pub fn canonical(text: &str) -> Result<String> { Ok(parse(text)?.iter().map(ToString::to_string).collect::<Vec<_>>().join(" ")) }
pub fn profile_targets(hardware: &impl Hardware, mut targets: Vec<(String,String)>) -> Vec<(String,String)> {
    // Other ROM layouts keep the ordinary bounded cpufreq path.
    if hardware.read(POWERHAL).and_then(|text| parse(&text)).is_err() { return targets; }
    let mut policies: Vec<_> = targets.iter().filter_map(|(path,_)| path.strip_prefix(&format!("{CPU}/"))?.strip_suffix("/scaling_min_freq").map(str::to_owned)).collect();
    policies.sort_by_key(|policy| policy.trim_start_matches("policy").parse::<u32>().unwrap_or(u32::MAX));
    if policies.len() != 3 { return targets; }
    let mut values = Vec::new();
    for policy in policies {
        for field in ["scaling_min_freq", "scaling_max_freq"] {
            let path = format!("{CPU}/{policy}/{field}");
            let Some((_,value)) = targets.iter().find(|(node,_)| *node == path) else { return targets; };
            values.push(value.clone());
        }
    }
    let request = values.join(" ");
    if parse(&request).is_err() { return targets; }
    targets.retain(|(path,_)| !path.starts_with(&format!("{CPU}/")));
    targets.insert(0, (POWERHAL.into(), request));
    targets
}
