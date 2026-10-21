use butter::{android::{display_rates, foreground, parse_setting, valid_package}, presets::Mode, runtime::{busy, cpu_counters, decide, parse_frames, AppRule, Config, Context, Lease, RuntimeState}};
fn context() -> Context { Context { awake:Some(true), saver:Some(false), temperature:Some(350), battery:Some(70), soc_millic:Some(45000),gpu_busy:Some(0.0),gpu_source:Some("fixture".into()),online:Some(false), user:Some(0), app:Some("com.example.game".into()), rates:vec![60,120], hz:Some(120.0) } }
fn state() -> RuntimeState { RuntimeState { tuning_requested:true, selected:Mode::Performance, ..Default::default() } }
#[test]
fn overlapping_boosts_expire_independently_and_never_survive_unknown_state() {
    let cfg = Config::default(); let mut state = state(); let mut ctx = context();
    state.foreground=ctx.app.clone(); state.leases = vec![Lease{source:"launch".into(),expires:103},Lease{source:"manual".into(),expires:110}];
    assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Boost);
    assert_eq!(state.leases.len(),1);
    assert_eq!(decide(&cfg,&mut state,&ctx,110).0,Mode::Performance);
    state.leases.push(Lease{source:"manual".into(),expires:115}); ctx.awake=None;
    assert_eq!(decide(&cfg,&mut state,&ctx,111).0,Mode::Balanced); assert!(state.leases.is_empty());
}
#[test]
fn monitoring_does_not_implicitly_enable_battery_tuning() {
    let mut state = RuntimeState::default(); let mut ctx = context(); ctx.battery=Some(5);
    assert_eq!(decide(&Config::default(),&mut state,&ctx,100).0,Mode::Balanced);
}
#[test]
fn heat_hysteresis_and_battery_saver_override_performance() {
    let cfg=Config::default(); let mut state=state(); let mut ctx=context();
    ctx.temperature=Some(425); let decision=decide(&cfg,&mut state,&ctx,100); assert_eq!(decision.0,Mode::Balanced); state.reason=decision.1.into();
    ctx.temperature=Some(405); assert_eq!(decide(&cfg,&mut state,&ctx,102).0,Mode::Balanced);
    ctx.temperature=Some(385); assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Performance);
    ctx.saver=Some(true); assert_eq!(decide(&cfg,&mut state,&ctx,106).0,Mode::Battery);
    ctx.saver=Some(false); ctx.temperature=None; assert_eq!(decide(&cfg,&mut state,&ctx,108).0,Mode::Balanced);
}
#[test]
fn automatic_launch_boost_only_follows_a_configured_performance_app() {
    let cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Performance,hz:Some(120)}],..Default::default()};
    let mut state=RuntimeState::default(); let mut ctx=context();
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Boost);
    state.foreground=ctx.app.clone(); state.leases.clear();
    assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Performance);
    ctx.app=Some("com.example.other".into()); assert_eq!(decide(&cfg,&mut state,&ctx,106).0,Mode::Balanced);
}
#[test]
fn foreground_parser_refuses_ambiguous_split_screen_and_untrusted_tokens() {
    assert_eq!(foreground("topResumedActivity=ActivityRecord{123 u0 com.example.game/.Main t4}"),Some("com.example.game".into()));
    assert_eq!(foreground("mResumedActivity: ActivityRecord{123 u0 com.one.app/.Main}\nmResumedActivity: ActivityRecord{456 u0 com.two.app/.Main}"),None);
    assert!(!valid_package("com.foo;reboot")); assert!(!valid_package("com..foo"));
    assert!(parse_setting("android:0:peak_refresh_rate").is_some()); assert!(parse_setting("android:0:peak_refresh_rate:extra").is_none());
}
#[test]
fn display_parser_only_accepts_advertised_internal_modes() {
    let dump="DisplayDeviceInfo{built-in, supportedModes [{id=1, width=1220, height=2712, fps=60.000004}, {id=2, fps=120.0}], type INTERNAL}\nDisplayDeviceInfo{external, supportedModes [{id=1, fps=90.0}], type EXTERNAL}";
    assert_eq!(display_rates(dump),vec![60,120]); assert!(display_rates("120 fps somewhere").is_empty());
}
#[test]
fn idle_workload_releases_floors_and_active_work_resumes_them() {
    let mut state=state(); let cfg=Config::default(); let ctx=context(); state.telemetry=serde_json::json!({"cpuBusyPercent":2.0});
    for now in [100,102] { assert_eq!(decide(&cfg,&mut state,&ctx,now).0,Mode::Performance); }
    assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Balanced);
    state.telemetry=serde_json::json!({"cpuBusyPercent":75.0}); assert_eq!(decide(&cfg,&mut state,&ctx,106).0,Mode::Performance);
}
#[test]
fn counter_wrap_and_frame_sentinels_do_not_become_performance_claims() {
    assert_eq!(cpu_counters("cpu 10 0 10 80 0 0 0 0\n"),Some((100,80)));
    assert_eq!(busy((100,80),(200,130)),Some(50.0)); assert_eq!(busy((200,130),(100,80)),None);
    let sample=parse_frames("Flags,IntendedVsync,Vsync,FrameCompleted,\n0,1000000,1000000,17000000,\n1,2000000,2000000,18000000,\n0,3000000,3000000,9223372036854775807,\n0,1000000,1000000,17000000,");
    assert_eq!(sample["frames"],1); assert_eq!(sample["p95Ms"],16.0);
}
#[test]
fn configuration_rejects_duplicate_packages_and_unsupported_modes() {
    let mut cfg=Config{rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Performance,hz:Some(500)}],..Default::default()}; assert!(cfg.validate().is_err());
    cfg.rules[0].hz=Some(120); cfg.rules.push(cfg.rules[0].clone()); assert!(cfg.validate().is_err());
}

#[test]
fn gpu_busy_prevents_cpu_only_idle_decisions_and_soc_heat_cancels_boosts() {
    let mut state=state(); let cfg=Config::default(); let mut ctx=context(); ctx.gpu_busy=Some(90.0); state.telemetry=serde_json::json!({"cpuBusyPercent":1.0});
    for now in [100,102,104,106] { assert_eq!(decide(&cfg,&mut state,&ctx,now).0,Mode::Performance); }
    ctx.soc_millic=Some(90000); state.leases.push(Lease{source:"manual".into(),expires:120});
    assert_eq!(decide(&cfg,&mut state,&ctx,108).0,Mode::Balanced); assert!(state.leases.is_empty());
}
#[test]
fn physical_refresh_reader_uses_active_mode_instead_of_render_rate() {
    let dump="DisplayDeviceInfo{built-in, modeId 2, renderFrameRate 60.0, supportedModes [{id=1, width=1220, height=2712, fps=60.0}, {id=2, width=1220, height=2712, fps=120.0}], type INTERNAL}";
    assert_eq!(butter::runtime::active_hz(dump),Some(120.0));
}

#[test]
fn leaving_a_game_cancels_its_launch_lease() {
    let cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Performance,hz:None}],..Default::default()};
    let mut state=RuntimeState::default(); let mut ctx=context();
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Boost);
    state.foreground=ctx.app.clone(); ctx.app=Some("com.example.other".into());
    assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Balanced); assert!(state.leases.is_empty());
}

#[test]
fn display_modes_survive_nested_hdr_and_alternative_rate_arrays() {
    let dump = "DisplayDeviceInfo{built-in, modeId 2, supportedModes [{id=1, fps=60.000004, alternativeRefreshRates=[120.0], supportedHdrTypes=[2, 3]}, {id=2, fps=120.00001, alternativeRefreshRates=[60.0], supportedHdrTypes=[2, 3]}], colorModes=[fps=90.0], type INTERNAL}";
    assert_eq!(display_rates(dump), vec![60,120]);
    assert_eq!(butter::runtime::active_hz(dump), Some(120.00001));
    assert!(display_rates(&dump.replace("type INTERNAL", "type EXTERNAL")).is_empty());
    assert!(display_rates("DisplayDeviceInfo{supportedModes [{id=1, fps=120, alternativeRefreshRates=[]}, type INTERNAL}").is_empty());
}

#[test]
fn low_refresh_cap_survives_cpu_conflicts_heat_and_saver() {
    let mut cfg = Config { automatic:true, rules:vec![AppRule { package:"com.example.game".into(), preset:Mode::Performance, hz:Some(60) }], ..Default::default() };
    let mut state = state(); state.blocked = true;
    let mut ctx = context(); ctx.soc_millic = Some(90000); ctx.saver = Some(true);
    assert_eq!(butter::runtime::requested_refresh(&cfg, &state, &ctx), Some(60));
    cfg.rules[0].hz = Some(120);
    assert_eq!(butter::runtime::requested_refresh(&cfg, &state, &ctx), None);
    cfg.rules[0].hz = Some(60); state.refresh_blocked = true;
    assert_eq!(butter::runtime::requested_refresh(&cfg, &state, &ctx), None);
    state.refresh_blocked = false; ctx.app = Some("com.other.app".into());
    assert_eq!(butter::runtime::requested_refresh(&cfg, &state, &ctx), None);
}
#[test]
fn event_fallback_respects_boost_and_advanced_deadlines() {
    let mut state = RuntimeState::default();
    assert_eq!(butter::runtime::next_wait(&state, true, 100).as_secs(), 5);
    state.leases.push(Lease { source:"launch".into(), expires:103 });
    assert_eq!(butter::runtime::next_wait(&state, false, 100).as_secs(), 3);
    state.advanced_deadlines.insert("gpu-lock".into(), 101);
    assert_eq!(butter::runtime::next_wait(&state, true, 100).as_secs(), 1);
}
#[test]
fn repeated_foreground_events_do_not_extend_the_launch_boost() {
    let cfg = Config { automatic:true, rules:vec![AppRule { package:"com.example.game".into(), preset:Mode::Performance, hz:None }], ..Default::default() };
    let mut state = RuntimeState::default(); let ctx = context();
    assert_eq!(decide(&cfg, &mut state, &ctx, 100).0, Mode::Boost);
    state.foreground = ctx.app.clone();
    assert_eq!(decide(&cfg, &mut state, &ctx, 101).0, Mode::Boost);
    assert_eq!(state.leases[0].expires, 103);
    assert_eq!(decide(&cfg, &mut state, &ctx, 104).0, Mode::Performance);
}

#[test]
fn old_settings_load_without_saved_controls_and_invalid_intents_are_rejected() {
    use butter::preferences::{SavedControl,validate};
    use std::collections::BTreeMap;
    let config:Config=serde_json::from_str(r#"{"automatic":false,"preset":"performance","logcat":true,"rules":[]}"#).unwrap();
    config.validate().unwrap(); assert!(config.controls.is_empty());
    assert!(validate(&BTreeMap::from([("cpu-lock".into(),SavedControl{setting:String::new(),expires_utc:None})])).is_err());
    assert!(validate(&BTreeMap::from([("profile".into(),SavedControl{setting:String::new(),expires_utc:None})])).is_err());
    assert!(validate(&BTreeMap::from([("governor-../../boot".into(),SavedControl{setting:"schedutil".into(),expires_utc:None})])).is_err());
}
