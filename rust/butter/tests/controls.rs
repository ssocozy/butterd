use butter::{engine::Engine, hardware::{Hardware, CPU, GOODIX, TCP, TCP_AVAILABLE}, journal::{decode_legacy, DiskStore, Journal, Store}, model::{choose_clock, touch_value, Feature}, Result};
use std::{cell::{Cell, RefCell}, collections::BTreeMap};
const BOOT_ID: &str = "11111111-2222-3333-4444-555555555555";
#[derive(Default)]
struct FakeHardware { nodes: RefCell<BTreeMap<String,String>>, writes: Cell<usize>, fail_at: Cell<Option<usize>> }
impl FakeHardware {
    fn put(&self, path: &str, value: &str) { self.nodes.borrow_mut().insert(path.into(), value.into()); }
}
impl Hardware for FakeHardware {
    fn read(&self, path: &str) -> Result<String> { self.nodes.borrow().get(path).cloned().ok_or_else(|| format!("Missing fixture {path}").into()) }
    fn write(&self, path: &str, value: &str) -> Result<()> {
        let number = self.writes.get()+1; self.writes.set(number);
        if self.fail_at.get() == Some(number) { return Err("Simulated write failure".into()); }
        self.put(path,value); Ok(())
    }
    fn list(&self, path: &str) -> Result<Vec<String>> {
        let prefix = format!("{path}/");
        let mut names: Vec<_> = self.nodes.borrow().keys().filter_map(|node| node.strip_prefix(&prefix)?.split('/').next().map(str::to_owned)).collect();
        names.sort(); names.dedup(); Ok(names)
    }
}
#[derive(Default)]
struct MemoryStore { saved: RefCell<Option<Journal>>, fail_save: Cell<bool> }
impl Store for MemoryStore {
    fn load(&self, _: &Feature) -> Result<Option<Journal>> { Ok(self.saved.borrow().clone()) }
    fn save(&self, journal: &Journal) -> Result<()> {
        if self.fail_save.get() { return Err("Simulated disk failure".into()); }
        *self.saved.borrow_mut() = Some(journal.clone()); Ok(())
    }
    fn remove(&self, _: &Feature) -> Result<()> { *self.saved.borrow_mut() = None; Ok(()) }
}
fn fixture() -> Engine<FakeHardware,MemoryStore> {
    let engine = Engine { hardware: FakeHardware::default(), store: MemoryStore::default(), boot: BOOT_ID.into() };
    for policy in ["policy0","policy4"] {
        for (field,value) in [("scaling_min_freq","480000"),("scaling_max_freq","1600000"),("scaling_available_frequencies","480000 1200000 2200000"),
            ("scaling_governor","schedutil"),("scaling_available_governors","schedutil powersave performance"),("related_cpus","0 1 2 3")] {
            engine.hardware.put(&format!("{CPU}/{policy}/{field}"),value);
        }
    }
    engine.hardware.put(GOODIX,"0"); engine.hardware.put(TCP,"cubic"); engine.hardware.put(TCP_AVAILABLE,"reno cubic bbr westwood"); engine
}
#[test]
fn advertised_clocks_are_bounded_and_malformed_tables_fail() {
    assert_eq!(choose_clock("480000 1200000 2200000",480000,1600000).unwrap(),1200000);
    assert!(choose_clock("2200000",0,1000000).is_err());
    assert!(choose_clock("480000 unknown",0,1600000).is_err());
    assert!(choose_clock("18446744073709551616",0,u64::MAX).is_err());
    assert!(choose_clock("480000",900000,100000).is_err());
}
#[test]
fn touch_does_not_confuse_rounded_or_ambiguous_rates() {
    assert_eq!(touch_value("480Hz").unwrap(),"1");
    assert!(touch_value("240Hz 480Hz").is_err()); assert!(touch_value("500Hz").is_err());
}
#[test]
fn advanced_opt_in_is_enforced_before_writes() {
    let engine = fixture(); assert!(engine.apply(Feature::CpuLock,"",false,false).is_err()); assert_eq!(engine.hardware.writes.get(),0);
}
#[test]
fn interrupted_multi_node_write_can_restore_every_original() {
    let engine = fixture(); engine.hardware.fail_at.set(Some(2));
    assert!(engine.apply(Feature::CpuLock,"",false,true).is_err());
    assert_eq!(engine.state(&Feature::CpuLock).unwrap(),"Interrupted; restore needed");
    engine.hardware.fail_at.set(None); engine.apply(Feature::CpuLock,"",true,false).unwrap();
    for policy in ["policy0","policy4"] { assert_eq!(engine.hardware.read(&format!("{CPU}/{policy}/scaling_min_freq")).unwrap(),"480000"); }
    assert!(engine.store.saved.borrow().is_none());
}
#[test]
fn disk_failure_prevents_any_kernel_write() {
    let engine = fixture(); engine.store.fail_save.set(true);
    assert!(engine.apply(Feature::Touch,"480",false,false).is_err()); assert_eq!(engine.hardware.writes.get(),0);
}
#[test]
fn external_changes_are_not_overwritten_during_restore() {
    let engine = fixture(); engine.apply(Feature::Touch,"480",false,false).unwrap(); engine.hardware.put(GOODIX,"0");
    let writes = engine.hardware.writes.get(); assert!(engine.apply(Feature::Touch,"",true,false).is_err()); assert_eq!(engine.hardware.writes.get(),writes);
}
#[test]
fn stale_boot_does_not_restore_older_values() {
    let mut engine = fixture(); engine.apply(Feature::Touch,"480",false,false).unwrap();
    engine.boot = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee".into(); let writes = engine.hardware.writes.get();
    engine.apply(Feature::Touch,"",true,false).unwrap(); assert_eq!(engine.hardware.writes.get(),writes);
}
#[test]
fn malicious_journal_paths_are_rejected_before_writing() {
    let engine = fixture(); engine.apply(Feature::Touch,"480",false,false).unwrap();
    engine.store.saved.borrow_mut().as_mut().unwrap().entries[0].path = "/etc/passwd".into();
    let writes = engine.hardware.writes.get(); assert!(engine.apply(Feature::Touch,"",true,false).is_err()); assert_eq!(engine.hardware.writes.get(),writes);
}
#[test]
fn tcp_choices_use_kernel_capabilities_and_restore() {
    let engine = fixture(); assert!(engine.apply(Feature::Tcp,"bbr2",false,false).is_err()); assert_eq!(engine.hardware.writes.get(),0);
    engine.apply(Feature::Tcp,"bbr",false,false).unwrap(); assert_eq!(engine.hardware.read(TCP).unwrap(),"bbr");
    engine.apply(Feature::Tcp,"",true,false).unwrap(); assert_eq!(engine.hardware.read(TCP).unwrap(),"cubic");
}
#[test]
fn governor_selection_and_original_performance_governor_are_preserved() {
    let engine = fixture(); let node = format!("{CPU}/policy0/scaling_governor"); engine.hardware.put(&node,"performance");
    engine.apply(Feature::Governor("policy0".into()),"powersave",false,false).unwrap();
    engine.apply(Feature::Governor("policy0".into()),"",true,false).unwrap(); assert_eq!(engine.hardware.read(&node).unwrap(),"performance");
}
#[test]
fn legacy_java_and_shell_sessions_migrate_without_discarding_originals() {
    let engine = fixture(); let legacy = format!("#Butterd session journal\nboot={BOOT_ID}\ncount=1\npath.0={GOODIX}\noriginal.0=0\nexpected.0=1\npending.0=true\nbefore.0=0\n");
    *engine.store.saved.borrow_mut() = Some(decode_legacy(&Feature::Touch,&legacy).unwrap());
    engine.apply(Feature::Touch,"",true,false).unwrap();
    let governor = decode_legacy(&Feature::Governor("policy0".into()),&format!("{BOOT_ID} performance powersave\n")).unwrap();
    assert_eq!(governor.entries[0].original,"performance");
}
#[test]
fn disk_store_locks_atomically_and_releases_on_drop() {
    let path = std::env::temp_dir().join(format!("butter-lock-test-{}",std::process::id())); std::fs::create_dir_all(&path).unwrap();
    let store = DiskStore::locked(&path).unwrap(); assert!(DiskStore::locked(&path).is_err());
    drop(store); let second = DiskStore::locked(&path).unwrap(); drop(second); std::fs::remove_dir_all(&path).unwrap();
}
#[test]
fn readonly_status_does_not_create_session_files() {
    let path = std::env::temp_dir().join(format!("butter-readonly-test-{}",std::process::id()));
    let store = DiskStore::new(&path); assert!(store.load(&Feature::Touch).unwrap().is_none()); assert!(!path.exists());
}
