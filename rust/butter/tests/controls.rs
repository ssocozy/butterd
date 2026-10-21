use butter::{
    engine::Engine,
    hardware::{Hardware, CPU, GOODIX, TCP, TCP_AVAILABLE},
    journal::{decode_legacy, DiskStore, Journal, Store},
    model::{choose_clock, touch_value, Feature},
    Result,
};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
};
const BOOT_ID: &str = "11111111-2222-3333-4444-555555555555";
#[derive(Default)]
struct FakeHardware {
    nodes: RefCell<BTreeMap<String, String>>,
    writes: Cell<usize>,
    fail_at: Cell<Option<usize>>,
}
impl FakeHardware {
    fn put(&self, path: &str, value: &str) {
        self.nodes.borrow_mut().insert(path.into(), value.into());
    }
}
impl Hardware for FakeHardware {
    fn read(&self, path: &str) -> Result<String> {
        self.nodes
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Missing fixture {path}").into())
    }
    fn write(&self, path: &str, value: &str) -> Result<()> {
        let number = self.writes.get() + 1;
        self.writes.set(number);
        if self.fail_at.get() == Some(number) {
            return Err("Simulated write failure".into());
        }
        if let Some((base, field)) = path.rsplit_once('/') {
            if let Ok(value) = value.parse::<u64>() {
                let other = match field { "scaling_min_freq" => Some(("scaling_max_freq", true)), "scaling_max_freq" => Some(("scaling_min_freq", false)), "min_freq" => Some(("max_freq", true)), "max_freq" => Some(("min_freq", false)), _ => None };
                if let Some((other, minimum)) = other {
                    if let Ok(bound) = self.read(&format!("{base}/{other}")).and_then(|value| value.parse::<u64>().map_err(Into::into)) {
                        if (minimum && value > bound) || (!minimum && value < bound) { return Err("Inverted fake hardware frequency limits".into()); }
                    }
                }
            }
        }
        self.put(path, value);
        Ok(())
    }
    fn list(&self, path: &str) -> Result<Vec<String>> {
        let prefix = format!("{path}/");
        let mut names: Vec<_> = self
            .nodes
            .borrow()
            .keys()
            .filter_map(|node| {
                node.strip_prefix(&prefix)?
                    .split('/')
                    .next()
                    .map(str::to_owned)
            })
            .collect();
        names.sort();
        names.dedup();
        Ok(names)
    }
}
#[derive(Default)]
struct MemoryStore {
    saved: RefCell<Option<Journal>>,
    fail_save: Cell<bool>,
}
impl Store for MemoryStore {
    fn load(&self, feature: &Feature) -> Result<Option<Journal>> {
        Ok(self.saved.borrow().clone().filter(|journal| &journal.feature == feature))
    }
    fn save(&self, journal: &Journal) -> Result<()> {
        if self.fail_save.get() {
            return Err("Simulated disk failure".into());
        }
        *self.saved.borrow_mut() = Some(journal.clone());
        Ok(())
    }
    fn remove(&self, _: &Feature) -> Result<()> {
        *self.saved.borrow_mut() = None;
        Ok(())
    }
}
fn fixture() -> Engine<FakeHardware, MemoryStore> {
    let engine = Engine {
        hardware: FakeHardware::default(),
        store: MemoryStore::default(),
        boot: BOOT_ID.into(),
    };
    for policy in ["policy0", "policy4"] {
        for (field, value) in [
            ("scaling_min_freq", "480000"),
            ("scaling_max_freq", "1600000"),
            ("scaling_available_frequencies", "480000 1200000 2200000"),
            ("scaling_governor", "schedutil"),
            (
                "scaling_available_governors",
                "schedutil powersave performance",
            ),
            ("related_cpus", "0 1 2 3"),
        ] {
            engine
                .hardware
                .put(&format!("{CPU}/{policy}/{field}"), value);
        }
    }
    engine.hardware.put(GOODIX, "0");
    engine.hardware.put(TCP, "cubic");
    engine
        .hardware
        .put(TCP_AVAILABLE, "reno cubic bbr westwood");
    engine
}
#[test]
fn advertised_clocks_are_bounded_and_malformed_tables_fail() {
    assert_eq!(
        choose_clock("480000 1200000 2200000", 480000, 1600000).unwrap(),
        1200000
    );
    assert!(choose_clock("2200000", 0, 1000000).is_err());
    assert!(choose_clock("480000 unknown", 0, 1600000).is_err());
    assert!(choose_clock("18446744073709551616", 0, u64::MAX).is_err());
    assert!(choose_clock("480000", 900000, 100000).is_err());
}
#[test]
fn touch_does_not_confuse_rounded_or_ambiguous_rates() {
    assert_eq!(touch_value("480Hz").unwrap(), "1");
    assert!(touch_value("240Hz 480Hz").is_err());
    assert!(touch_value("500Hz").is_err());
}
#[test]
fn advanced_opt_in_is_enforced_before_writes() {
    let engine = fixture();
    assert!(engine.apply(Feature::CpuLock, "", false, false).is_err());
    assert_eq!(engine.hardware.writes.get(), 0);
}
#[test]
fn interrupted_multi_node_write_can_restore_every_original() {
    let engine = fixture();
    engine.hardware.fail_at.set(Some(2));
    assert!(engine.apply(Feature::CpuLock, "", false, true).is_err());
    assert_eq!(
        engine.state(&Feature::CpuLock).unwrap(),
        "Interrupted; restore needed"
    );
    engine.hardware.fail_at.set(None);
    engine.apply(Feature::CpuLock, "", true, false).unwrap();
    for policy in ["policy0", "policy4"] {
        assert_eq!(
            engine
                .hardware
                .read(&format!("{CPU}/{policy}/scaling_min_freq"))
                .unwrap(),
            "480000"
        );
    }
    assert!(engine.store.saved.borrow().is_none());
}
#[test]
fn disk_failure_prevents_any_kernel_write() {
    let engine = fixture();
    engine.store.fail_save.set(true);
    assert!(engine.apply(Feature::Touch, "480", false, false).is_err());
    assert_eq!(engine.hardware.writes.get(), 0);
}
#[test]
fn external_changes_are_not_overwritten_during_restore() {
    let engine = fixture();
    engine.apply(Feature::Tcp, "bbr", false, false).unwrap();
    // Neither the saved original (cubic) nor butter's requested value (bbr).
    engine.hardware.put(TCP, "westwood");
    let writes = engine.hardware.writes.get();
    assert!(engine.apply(Feature::Tcp, "", true, false).is_err());
    assert_eq!(engine.hardware.writes.get(), writes);
    assert_eq!(engine.hardware.read(TCP).unwrap(), "westwood");
}
#[test]
fn stale_boot_does_not_restore_older_values() {
    let mut engine = fixture();
    engine.apply(Feature::Touch, "480", false, false).unwrap();
    engine.boot = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into();
    let writes = engine.hardware.writes.get();
    engine.apply(Feature::Touch, "", true, false).unwrap();
    assert_eq!(engine.hardware.writes.get(), writes);
}
#[test]
fn malicious_journal_paths_are_rejected_before_writing() {
    let engine = fixture();
    engine.apply(Feature::Touch, "480", false, false).unwrap();
    engine.store.saved.borrow_mut().as_mut().unwrap().entries[0].path = "/etc/passwd".into();
    let writes = engine.hardware.writes.get();
    assert!(engine.apply(Feature::Touch, "", true, false).is_err());
    assert_eq!(engine.hardware.writes.get(), writes);
}
#[test]
fn tcp_choices_use_kernel_capabilities_and_restore() {
    let engine = fixture();
    assert!(engine.apply(Feature::Tcp, "bbr2", false, false).is_err());
    assert_eq!(engine.hardware.writes.get(), 0);
    engine.apply(Feature::Tcp, "bbr", false, false).unwrap();
    assert_eq!(engine.hardware.read(TCP).unwrap(), "bbr");
    engine.apply(Feature::Tcp, "", true, false).unwrap();
    assert_eq!(engine.hardware.read(TCP).unwrap(), "cubic");
}
#[test]
fn governor_selection_and_original_performance_governor_are_preserved() {
    let engine = fixture();
    let node = format!("{CPU}/policy0/scaling_governor");
    engine.hardware.put(&node, "performance");
    engine
        .apply(
            Feature::Governor("policy0".into()),
            "powersave",
            false,
            false,
        )
        .unwrap();
    engine
        .apply(Feature::Governor("policy0".into()), "", true, false)
        .unwrap();
    assert_eq!(engine.hardware.read(&node).unwrap(), "performance");
}
#[test]
fn legacy_java_and_shell_sessions_migrate_without_discarding_originals() {
    let engine = fixture();
    let legacy = format!("#Butterd session journal\nboot={BOOT_ID}\ncount=1\npath.0={GOODIX}\noriginal.0=0\nexpected.0=1\npending.0=true\nbefore.0=0\n");
    *engine.store.saved.borrow_mut() = Some(decode_legacy(&Feature::Touch, &legacy).unwrap());
    engine.apply(Feature::Touch, "", true, false).unwrap();
    let governor = decode_legacy(
        &Feature::Governor("policy0".into()),
        &format!("{BOOT_ID} performance powersave\n"),
    )
    .unwrap();
    assert_eq!(governor.entries[0].original, "performance");
}
#[test]
fn disk_store_locks_atomically_and_releases_on_drop() {
    let path = std::env::temp_dir().join(format!("butter-lock-test-{}", std::process::id()));
    std::fs::create_dir_all(&path).unwrap();
    let store = DiskStore::locked(&path).unwrap();
    assert!(DiskStore::locked(&path).is_err());
    drop(store);
    let second = DiskStore::locked(&path).unwrap();
    drop(second);
    std::fs::remove_dir_all(&path).unwrap();
}
#[test]
fn readonly_status_does_not_create_session_files() {
    let path = std::env::temp_dir().join(format!("butter-readonly-test-{}", std::process::id()));
    let store = DiskStore::new(&path);
    assert!(store.load(&Feature::Touch).unwrap().is_none());
    assert!(!path.exists());
}

#[test]
fn profiles_use_baseline_without_compounding_and_restore() {
    use butter::presets::Mode;
    let engine = fixture();
    engine.preset(Mode::Battery).unwrap();
    engine.preset(Mode::Performance).unwrap();
    assert_eq!(engine.hardware.read(&format!("{CPU}/policy0/scaling_max_freq")).unwrap(), "1600000");
    engine.preset(Mode::Balanced).unwrap();
    assert_eq!(engine.hardware.read(&format!("{CPU}/policy0/scaling_min_freq")).unwrap(), "480000");
    assert_eq!(engine.hardware.read(&format!("{CPU}/policy0/scaling_max_freq")).unwrap(), "1600000");
}
#[test]
fn failed_profile_rolls_back_earlier_nodes() {
    let engine = fixture();
    engine.hardware.fail_at.set(Some(2));
    assert!(engine.preset(butter::presets::Mode::Performance).is_err());
    assert_eq!(engine.hardware.read(&format!("{CPU}/policy0/scaling_min_freq")).unwrap(), "480000");
    assert!(engine.session(&Feature::Profile).unwrap().is_none());
}
#[test]
fn manual_ranges_cannot_overlap_an_active_profile() {
    let engine = fixture();
    engine.preset(butter::presets::Mode::Performance).unwrap();
    let writes = engine.hardware.writes.get();
    assert!(engine.apply(Feature::CpuRange("policy0".into()), "480000:1200000", false, false).is_err());
    assert_eq!(engine.hardware.writes.get(), writes);
}
#[test]
fn ranges_reject_overclock_and_inverted_limits_before_writes() {
    let engine = fixture();
    for range in ["1200000:480000", "480000:2200000", "0:1200000", "480000:;reboot"] {
        assert!(engine.apply(Feature::CpuRange("policy0".into()), range, false, false).is_err());
    }
    assert_eq!(engine.hardware.writes.get(), 0);
    engine.apply(Feature::CpuRange("policy0".into()), "480000:1200000", false, false).unwrap();
    engine.apply(Feature::CpuRange("policy0".into()), "", true, false).unwrap();
}
#[test]
fn bypass_requires_capability_control_and_observable_state() {
    use butter::hardware::{bypass_node, BYPASS_ACTIVE};
    let engine = fixture();
    engine.hardware.put("/sys/class/power_supply/battery/bypass_charging", "0");
    assert!(bypass_node(&engine.hardware).is_err());
    engine.hardware.put("/sys/class/power_supply/battery/bypass_charging_supported", "1");
    assert!(bypass_node(&engine.hardware).is_err());
    engine.hardware.put(BYPASS_ACTIVE, "0");
    assert!(bypass_node(&engine.hardware).is_ok());
}
#[test]
fn charging_pause_preserves_unlimited_flag_and_never_changes_current_limit() {
    let engine = fixture();
    engine.hardware.put(butter::hardware::PAUSE, "1 0");
    engine.apply(Feature::ChargingPause, "", false, true).unwrap();
    assert_eq!(engine.hardware.read(butter::hardware::PAUSE).unwrap(), "1 1");
    engine.apply(Feature::ChargingPause, "", true, false).unwrap();
    assert_eq!(engine.hardware.read(butter::hardware::PAUSE).unwrap(), "1 0");
    assert_eq!(engine.hardware.writes.get(), 2);
}
#[test]
fn refresh_journals_cannot_target_another_user_or_non_display_setting() {
    let engine = fixture();
    for path in ["android:10:peak_refresh_rate", "android:0:screen_brightness", "/system/build.prop"] {
        assert!(engine.apply_targets(Feature::Refresh(0), vec![(path.into(), "120".into())], false).is_err());
    }
    assert_eq!(engine.hardware.writes.get(), 0);
}

#[test]
fn display_journal_remains_restorable_after_reboot() {
    let mut engine = fixture();
    let path = "android:0:peak_refresh_rate";
    engine.hardware.put(path, "60");
    engine.apply_targets(Feature::Refresh(0),vec![(path.into(),"120".into())],false).unwrap();
    engine.boot="aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into();
    engine.apply_targets(Feature::Refresh(0),Vec::new(),true).unwrap();
    assert_eq!(engine.hardware.read(path).unwrap(),"60");
}

#[test]
fn shared_lock_backend_distinguishes_contention_from_errors() {
    use butter::lock::FileLock;
    let path = std::env::temp_dir().join(format!("butter-portable-lock-{}",std::process::id()));
    assert!(!FileLock::held(&path).unwrap());
    let first = FileLock::acquire(&path).unwrap().unwrap();
    assert!(FileLock::held(&path).unwrap());
    assert!(FileLock::acquire(&path).unwrap().is_none());
    drop(first);
    assert!(!FileLock::held(&path).unwrap());
    std::fs::remove_file(path).unwrap();
}

#[test]
fn touch_returning_to_original_releases_session_without_a_write() {
    let engine = fixture();
    engine.apply(Feature::Touch, "480", false, false).unwrap();
    engine.hardware.put(GOODIX, "0");
    let writes = engine.hardware.writes.get();
    assert_eq!(engine.state(&Feature::Touch).unwrap(), "Original values active; release session");
    engine.apply(Feature::Touch, "", true, false).unwrap();
    assert_eq!(engine.hardware.writes.get(), writes);
    assert!(engine.session(&Feature::Touch).unwrap().is_none());
}

fn vendor_fixture() -> Engine<FakeHardware,MemoryStore> {
    let engine = fixture();
    for (field,value) in [("scaling_min_freq","480000"),("scaling_max_freq","1600000"),("scaling_available_frequencies","480000 800000 1200000 1600000") ] {
        engine.hardware.put(&format!("{CPU}/policy7/{field}"),value);
    }
    engine.hardware.put(butter::vendor::POWERHAL,"-1 -1 -1 -1 -1 -1");
    engine
}
#[test]
fn vendor_qos_uses_all_clusters_and_preserves_cpu_baseline() {
    use butter::presets::Mode;
    let engine = vendor_fixture();
    engine.preset(Mode::Performance).unwrap();
    let journal = engine.session(&Feature::Profile).unwrap().unwrap();
    assert_eq!(journal.cpu_limits.len(),6);
    assert_eq!(journal.entries.len(),1);
    assert_eq!(journal.entries[0].path,butter::vendor::POWERHAL);
    engine.hardware.put(&format!("{CPU}/policy0/scaling_max_freq"),"1200000");
    let status = engine.status();
    assert_eq!(status["cpuPolicies"][0]["max"],"1200000");
    assert_eq!(status["cpuPolicies"][0]["requestedMax"],"1600000");
    assert_eq!(butter::vendor::parse(&journal.entries[0].expected).unwrap().len(),6);
    let first = engine.hardware.read(butter::vendor::POWERHAL).unwrap();
    engine.hardware.put(&format!("{CPU}/policy0/scaling_min_freq"),"1200000");
    engine.preset(Mode::Performance).unwrap();
    assert_eq!(engine.hardware.read(butter::vendor::POWERHAL).unwrap(),first);
    assert!(engine.apply(Feature::CpuRange("policy0".into()),"1200000:1200000",false,false).is_err());
    engine.preset(Mode::Balanced).unwrap();
    assert_eq!(engine.hardware.read(butter::vendor::POWERHAL).unwrap(),"-1 -1 -1 -1 -1 -1");
}
#[test]
fn vendor_reconcile_preserves_latest_rom_request_on_release() {
    use butter::presets::Mode;
    let engine = vendor_fixture();
    engine.preset(Mode::Performance).unwrap();
    let rom = "480000 1200000 -1 -1 -1 -1";
    engine.hardware.put(butter::vendor::POWERHAL,rom);
    engine.reconcile_vendor_profile(Mode::Performance).unwrap();
    assert_ne!(engine.hardware.read(butter::vendor::POWERHAL).unwrap(),rom);
    engine.preset(Mode::Balanced).unwrap();
    assert_eq!(engine.hardware.read(butter::vendor::POWERHAL).unwrap(),rom);
    assert!(butter::vendor::parse("0 1000000 2000000").is_err());
    assert!(butter::vendor::parse("0 0 0 0 0 0").is_err());
    assert!(butter::vendor::parse("-2 -1 -1 -1 -1 -1").is_err());
    assert!(butter::hardware::validate_path_value(&Feature::Profile,GOODIX,"-1 -1 -1 -1 -1 -1").is_err());
}

#[test]
fn infinity_and_rom_display_votes_apply_and_restore_without_manufacturing_keys() {
    let engine = fixture();
    for (path,value) in [("android:0:min_refresh_rate","0"),("android:0:peak_refresh_rate","Infinity"),("display:0:preferred","null"),("rom-refresh:0:system:user_refresh_rate","120"),("rom-refresh:0:system:miui_refresh_rate","120"),("rom-refresh:0:system:is_smart_fps","1")] { engine.hardware.put(path,value); }
    let dump = "DisplayDeviceInfo{supportedModes [{id=1, width=1220, height=2712, fps=60.0}, {id=2, width=1220, height=2712, fps=120.0}], type INTERNAL}";
    let targets = butter::display::targets(&engine.hardware,0,60,dump).unwrap();
    assert_eq!(targets.len(),6);
    assert!(!targets.iter().any(|(path,_)| path.contains(":secure:")));
    engine.apply_targets(Feature::Refresh(0),targets,false).unwrap();
    assert_eq!(engine.hardware.read("display:0:preferred").unwrap(),"1220 2712 60");
    assert_eq!(engine.hardware.read("android:0:peak_refresh_rate").unwrap(),"60");
    engine.hardware.put("rom-refresh:0:system:user_refresh_rate","90");
    engine.apply_targets(Feature::Refresh(0),Vec::new(),true).unwrap();
    assert_eq!(engine.hardware.read("android:0:peak_refresh_rate").unwrap(),"Infinity");
    assert_eq!(engine.hardware.read("android:0:min_refresh_rate").unwrap(),"0");
    assert_eq!(engine.hardware.read("display:0:preferred").unwrap(),"null");
    assert_eq!(engine.hardware.read("rom-refresh:0:system:user_refresh_rate").unwrap(),"90");
    assert_eq!(engine.hardware.read("rom-refresh:0:system:is_smart_fps").unwrap(),"1");
    assert!(butter::display::normalize("system","min_refresh_rate","Infinity").is_err());
    assert!(butter::display::normalize("system","peak_refresh_rate","NaN").is_err());
    assert!(butter::display::node("rom-refresh:0:global:miui_refresh_rate").is_none());
    assert!(butter::hardware::validate_path_value(&Feature::Refresh(10),"display:0:preferred","null").is_err());
    assert_eq!(butter::display::preferred_reply("User preferred display mode: null").unwrap(),"null");
}

#[test]
fn android_unset_preferred_mode_is_captured_and_released() {
    let engine = fixture();
    let original = butter::display::preferred_reply("User preferred display mode: -1 -1 0.0\r\n").unwrap();
    assert_eq!(original,"null");
    for (path,value) in [("display:0:preferred",original.as_str()),("android:0:min_refresh_rate","0"),("android:0:peak_refresh_rate","Infinity")] { engine.hardware.put(path,value); }
    // Stock Android 16 output includes nested refresh and HDR lists.
    let dump = "DisplayDeviceInfo{modeId 2, supportedModes [{id=1, parentModeId=-1, sfModeId=0, flags=, width=1220, height=2712, fps=60.0, vsync=60.0, alternativeRefreshRates=[90.0, 120.00001], supportedHdrTypes=[2, 3, 4]}, {id=2, width=1220, height=2712, fps=120.00001, alternativeRefreshRates=[60.0, 90.0], supportedHdrTypes=[2, 3, 4]}], type INTERNAL}";
    let targets = butter::display::targets(&engine.hardware,0,60,dump).unwrap();
    assert_eq!(targets.len(),3);
    assert!(targets.contains(&("display:0:preferred".into(),"1220 2712 60".into())));
    engine.apply_targets(Feature::Refresh(0),targets,false).unwrap();
    assert_eq!(engine.hardware.read("display:0:preferred").unwrap(),"1220 2712 60");
    engine.apply_targets(Feature::Refresh(0),Vec::new(),true).unwrap();
    assert_eq!(engine.hardware.read("display:0:preferred").unwrap(),"null");
    assert_eq!(engine.hardware.read("android:0:peak_refresh_rate").unwrap(),"Infinity");
    for invalid in ["-1 -1 60", "-1 2712 0", "1220 -1 0", "-1 -1 NaN"] {
        assert!(butter::display::preferred_reply(&format!("User preferred display mode: {invalid}")).is_err());
    }
}

#[test]
fn unified_governor_applies_and_restores_each_clusters_original() {
    let engine = fixture();
    engine.hardware.put(&format!("{CPU}/policy4/scaling_governor"),"performance");
    engine.apply(Feature::Governors,"powersave",false,false).unwrap();
    for policy in ["policy0","policy4"] { assert_eq!(engine.hardware.read(&format!("{CPU}/{policy}/scaling_governor")).unwrap(),"powersave"); }
    assert!(engine.apply(Feature::Governor("policy0".into()),"schedutil",false,false).is_err());
    engine.apply(Feature::Governors,"",true,false).unwrap();
    assert_eq!(engine.hardware.read(&format!("{CPU}/policy0/scaling_governor")).unwrap(),"schedutil");
    assert_eq!(engine.hardware.read(&format!("{CPU}/policy4/scaling_governor")).unwrap(),"performance");
}
#[test]
fn unified_governor_preflights_all_clusters_and_rolls_back_failed_write() {
    let engine = fixture();
    engine.hardware.put(&format!("{CPU}/policy4/scaling_available_governors"),"schedutil");
    assert!(engine.apply(Feature::Governors,"powersave",false,false).is_err());
    assert_eq!(engine.hardware.writes.get(),0);
    engine.hardware.put(&format!("{CPU}/policy4/scaling_available_governors"),"schedutil powersave");
    engine.hardware.fail_at.set(Some(2));
    assert!(engine.apply(Feature::Governors,"powersave",false,false).is_err());
    for policy in ["policy0","policy4"] { assert_eq!(engine.hardware.read(&format!("{CPU}/{policy}/scaling_governor")).unwrap(),"schedutil"); }
}
#[test]
fn unified_governor_preserves_external_changes_and_checks_original_support() {
    let engine = fixture();
    engine.apply(Feature::Governors,"powersave",false,false).unwrap();
    engine.hardware.put(&format!("{CPU}/policy4/scaling_governor"),"performance");
    let writes = engine.hardware.writes.get();
    assert!(engine.apply(Feature::Governors,"",true,false).is_err());
    assert_eq!(engine.hardware.writes.get(),writes);
    engine.hardware.put(&format!("{CPU}/policy4/scaling_governor"),"powersave");
    engine.hardware.put(&format!("{CPU}/policy4/scaling_available_governors"),"powersave");
    assert!(engine.apply(Feature::Governors,"",true,false).is_err());
    assert_eq!(engine.hardware.writes.get(),writes);
}

#[test]
fn rom_app_refresh_restores_only_its_rule_and_keeps_other_apps() {
    let engine = fixture();
    let app = "com.mobile.legends";
    let path = "rom-app:0:refresh_rate_ext:com.voltage.display.IRefreshRateManagerService:com.mobile.legends";
    let other = "rom-app:0:refresh_rate_ext:com.voltage.display.IRefreshRateManagerService:com.other.game";
    engine.hardware.put(path,"120"); engine.hardware.put(other,"90");
    let adapter = serde_json::json!({"available":true,"service":"refresh_rate_ext","descriptor":"com.voltage.display.IRefreshRateManagerService"});
    let targets = butter::display::app_targets(&engine.hardware,0,app,60,&adapter).unwrap();
    engine.apply_targets(Feature::Refresh(0),targets,false).unwrap();
    assert_eq!(engine.hardware.read(path).unwrap(),"60");
    assert_eq!(engine.hardware.read(other).unwrap(),"90");
    engine.apply_targets(Feature::Refresh(0),Vec::new(),true).unwrap();
    assert_eq!(engine.hardware.read(path).unwrap(),"120");
    assert_eq!(engine.hardware.read(other).unwrap(),"90");
    assert!(butter::hardware::validate_path_value(&Feature::Refresh(10),path,"60").is_err());
    assert!(!butter::hardware::allowed(&engine.hardware,&Feature::Profile,path));
}
#[test]
fn rom_refresh_fallback_unset_rules_and_external_edits_are_preserved() {
    let engine = fixture();
    let path = "rom-app:0:refresh_rate_ext:com.voltage.display.IRefreshRateManagerService:com.mobile.legends";
    let adapter = serde_json::json!({"available":true,"service":"refresh_rate_ext","descriptor":"com.voltage.display.IRefreshRateManagerService"});
    engine.hardware.put(path,"-1");
    assert!(butter::display::app_targets(&engine.hardware,0,"com.mobile.legends",60,&serde_json::json!({"available":false})).unwrap().is_empty());
    let targets = butter::display::app_targets(&engine.hardware,0,"com.mobile.legends",60,&adapter).unwrap();
    engine.apply_targets(Feature::Refresh(0),targets,false).unwrap();
    engine.apply_targets(Feature::Refresh(0),Vec::new(),true).unwrap();
    assert_eq!(engine.hardware.read(path).unwrap(),"-1");
    engine.apply_targets(Feature::Refresh(0),vec![(path.into(),"60".into())],false).unwrap();
    engine.hardware.put(path,"90");
    engine.apply_targets(Feature::Refresh(0),Vec::new(),true).unwrap();
    assert_eq!(engine.hardware.read(path).unwrap(),"90");
    for value in ["null","NaN","-2","241","60.5"] { assert!(butter::display::normalize("app","com.mobile.legends",value).is_err()); }
    assert!(butter::display::app_node(&path.replace("refresh_rate_ext","activity")).is_none());
    assert!(butter::display::app_node(&path.replace("com.mobile.legends","../other")).is_none());
    assert!(butter::display::app_targets(&engine.hardware,0,"com.mobile.legends",60,&serde_json::json!({"available":true,"service":"activity","descriptor":"android.app.IActivityManager"})).is_err());
}

#[test]
fn ged_activity_is_a_read_only_fallback_and_missing_counters_stay_unknown() {
    let engine = fixture();
    assert_eq!(butter::gpu::activity(&engine.hardware),None);
    engine.hardware.put(butter::gpu::GED_UTILIZATION,"42 3 55");
    assert_eq!(butter::gpu::activity(&engine.hardware),Some((42.0,"MediaTek GED")));
    let load = format!("{}/load",butter::hardware::GPU.trim_end_matches("/min_freq"));
    engine.hardware.put(&load,"17%");
    assert_eq!(butter::gpu::activity(&engine.hardware),Some((17.0,"devfreq")));
    engine.hardware.put(&load,"NaN");
    assert_eq!(butter::gpu::activity(&engine.hardware),Some((42.0,"MediaTek GED")));
    engine.hardware.put(butter::gpu::GED_UTILIZATION,"0 0 0");
    assert_eq!(butter::gpu::activity(&engine.hardware),None);
    assert_eq!(engine.hardware.writes.get(),0);
}
