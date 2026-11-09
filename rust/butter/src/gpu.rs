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

pub const GED_FLOOR:&str="/sys/kernel/ged/hal/custom_boost_gpu_freq";
pub const GED_COUNT:&str="/sys/kernel/ged/hal/total_gpu_freq_level_count";
pub const GED_CURRENT:&str="/sys/kernel/ged/hal/current_freqency";
pub const OPP_TABLE:&str="/proc/gpufreqv2/gpu_working_opp_table";
pub const LIMIT_TABLE:&str="/proc/gpufreqv2/limit_table";

// MiCode MTK_kernel_device_modules, bsp-duchamp-u-oss:
// gpufreq/v2/gpufreq_debug.c limit_table_proc_write; gpuppm.c LIMIT_APIBOOST.
/// Virtual node: read/write only APIBOOST's floor, preserving its ceiling and flags.
pub const APIBOOST_FLOOR: &str = "vendor:gpu-apiboost-floor";
#[derive(Debug, PartialEq, Eq)]
pub struct ApiBoost { pub id: u32, pub floor: i32 }
pub fn apiboost(raw: &str) -> crate::Result<ApiBoost> {
    if raw.len()>8192 { return Err("GPU limiter table exceeds limit".into()); }
    let mut rows=std::collections::BTreeMap::new();
    let mut ids=std::collections::BTreeSet::new();
    for line in raw.lines().filter(|line| !line.trim().is_empty()) {
        let fields:Vec<_>=line.split_whitespace().collect();
        if fields.first()==Some(&"[id]") { continue; }
        if fields.len()!=7 { return Err("Unsupported GPU limiter table".into()); }
        let id=fields[0].parse::<u32>()?;
        let priority=fields[2].parse::<u32>()?;
        let ceiling=fields[3].parse::<i32>()?;
        let floor=fields[4].parse::<i32>()?;
        if id>99 || !(-1..=99).contains(&ceiling) || !(-1..=99).contains(&floor)
            || !matches!(fields[5],"0"|"1") || !matches!(fields[6],"0"|"1")
            || !ids.insert(id) || rows.insert(fields[1],(id,priority,floor,fields[5],fields[6])).is_some() {
            return Err("Invalid GPU limiter row".into());
        }
    }
    let api=rows.get("APIBOOST").ok_or("GPU APIBOOST limiter missing")?;
    let fpsgo=rows.get("FPSGO").ok_or("GPU FPSGO limiter missing")?;
    for name in ["THERMAL_AP","THERMAL_EB"] {
        let thermal=rows.get(name).ok_or("GPU thermal limiter missing")?;
        if thermal.1<=api.1 || thermal.3!="1" { return Err("GPU thermal priority is unsupported".into()); }
    }
    if api.1!=3 || fpsgo.1!=2 || api.4!="1" { return Err("GPU APIBOOST priority or enable flag unsupported".into()); }
    Ok(ApiBoost{id:api.0,floor:api.2})
}
pub fn floor_node(hardware: &impl Hardware) -> &'static str {
    if hardware.read(APIBOOST_FLOOR).is_ok() { APIBOOST_FLOOR } else { GED_FLOOR }
}
pub fn apiboost_command(raw: &str, value: &str) -> crate::Result<String> {
    let row=apiboost(raw)?;
    let floor=value.parse::<i32>()?;
    if !(-1..=99).contains(&floor) { return Err("Invalid APIBOOST floor".into()); }
    // -3 means KEEP. Never reset or raise the ceiling and never switch flags.
    Ok(format!("set {} -3 {floor}",row.id))
}

pub fn profile_node(path: &str) -> bool {
    path == GED_FLOOR || path == APIBOOST_FLOOR || [GPU,GPU_ALT].iter().any(|node| path==*node || path==node.replace("min_freq","max_freq"))
}

pub fn dummy(hardware:&impl Hardware)->bool {
    [GPU,GPU_ALT].iter().any(|node|hardware.read(&format!("{}/governor",node.trim_end_matches("/min_freq"))).is_ok_and(|name|name=="dummy"))
}
pub fn opp_table(raw:&str,available:&str,count:usize)->crate::Result<Vec<u64>> {
    if !(2..=256).contains(&count)||raw.len()>32768{return Err("Unsupported MediaTek OPP count".into());}
    let mut table=Vec::new();
    for line in raw.lines().map(str::trim).filter(|line|!line.is_empty()) {
        let (index,fields)=line.strip_prefix('[').and_then(|line|line.split_once(']')).ok_or("Unsupported MediaTek OPP row")?;
        if index.parse::<usize>()?!=table.len(){return Err("MediaTek OPP indices are not sequential".into());}
        let khz=fields.trim().strip_prefix("freq:").and_then(|fields|fields.split(',').next()).ok_or("Missing MediaTek OPP frequency")?.trim().parse::<u64>()?;
        let hz=khz.checked_mul(1000).filter(|value|(1..=4000000000).contains(value)).ok_or("Invalid GPU frequency")?;
        if table.last().is_some_and(|previous|*previous<=hz){return Err("MediaTek OPP order is unsupported".into());}table.push(hz);
    }
    let mut advertised=crate::presets::clocks(available,1,4000000000)?;advertised.reverse();
    if table.len()!=count||table!=advertised{return Err("MediaTek OPP map does not match advertised frequencies".into());}
    Ok(table)
}
pub fn ged_table(hardware:&impl Hardware)->crate::Result<Vec<u64>> {
    if !dummy(hardware){return Err("MediaTek dummy governor is not active".into());}
    let count=hardware.read(GED_COUNT)?.parse::<usize>()?;
    let node=if hardware.read(GPU).is_ok(){GPU}else{GPU_ALT};
    let table=opp_table(&hardware.read(OPP_TABLE)?,&hardware.read(&format!("{}/available_frequencies",node.trim_end_matches("/min_freq")))?,count)?;
    let floor=hardware.read(GED_FLOOR)?.parse::<usize>()?;
    if floor>=table.len(){return Err("GPU floor index is outside the working OPP table".into());}
    Ok(table)
}
pub fn floor_index(mode:crate::presets::Mode,stage:u8,count:usize)->crate::Result<usize> {
    use crate::presets::Mode;
    if !(2..=256).contains(&count)||stage>3{return Err("Invalid GPU profile table or cooling stage".into());}
    let last=count-1;
    let ascending=match mode {
        Mode::Aggressive=>match stage {0=>(last*3).div_ceil(4),1=>last/2,2=>last/4,_=>0},
        Mode::Battery|Mode::Balanced=>0,
        Mode::Performance|Mode::Boost=>match stage {0 if mode==Mode::Boost=>(last*2).div_ceil(3),0=>count/2,1=>last/3,2=>last/6,_=>0},
    };
    Ok(last-ascending)
}
pub fn ged_current(raw:&str)->Option<(usize,u64)> {
    let fields:Vec<_>=raw.split_whitespace().collect();if fields.len()!=2{return None;}
    let index=fields[0].parse::<usize>().ok()?;let hz=fields[1].parse::<u64>().ok()?.checked_mul(1000)?;
    if index>255||!(1..=4000000000).contains(&hz){None}else{Some((index,hz))}
}


#[cfg(test)]
mod tests {
    use super::parse_ged;
    #[test]
    fn apiboost_targets_its_own_slot_without_changing_thermal_limits() {
        let table="[id] [name] [priority] [ceiling] [floor] [c_enable] [f_enable]\n4 THERMAL_AP 6 47 -1 1 1\n5 THERMAL_EB 6 -1 -1 1 1\n11 APIBOOST 3 -1 64 1 1\n12 FPSGO 2 -1 0 1 1\n";
        assert_eq!(super::apiboost(table).unwrap(),super::ApiBoost{id:11,floor:64});
        assert_eq!(super::apiboost_command(table,"16").unwrap(),"set 11 -3 16");
        assert_eq!(super::apiboost_command(table,"-1").unwrap(),"set 11 -3 -1");
        assert_eq!(super::apiboost(&table.replace("11 APIBOOST","10 APIBOOST")).unwrap().id,10);
        for bad in [table.replace("APIBOOST 3","APIBOOST 8"), table.replace("47 -1 1 1","47 -1 0 1"),table.replace("64 1 1","64 1 0"),table.replace("11 APIBOOST","12 APIBOOST"),format!("{table}11 APIBOOST 3 -1 64 1 1\n")] {
            assert!(super::apiboost(&bad).is_err(),"{bad}");
        }
        for value in ["-2","-3","100","16 0","x"] { assert!(super::apiboost_command(table,value).is_err()); }
    }
    #[test]
    fn loading_is_the_first_counter_and_invalid_samples_stay_unknown() {
        assert_eq!(parse_ged("37 2 61\n"),Some(37.0));
        assert_eq!(parse_ged("0 0 100"),Some(0.0));
        assert_eq!(parse_ged("100 0 0"),Some(100.0));
        for raw in ["", "0 0 0", "37", "37%", "1 2", "1 2 3 4", "101 0 0", "1 101 0", "1 0 101", "-1 0 100", "NaN 0 100", "3.5 0 96", "1% 0 99"] { assert_eq!(parse_ged(raw),None,"{raw}"); }
    }
}
