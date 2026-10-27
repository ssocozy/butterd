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
    let cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Performance,hz:Some(120),high_touch:false,charging_pause_at:None}],..Default::default()};
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
    let mut cfg=Config{rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Performance,hz:Some(500),high_touch:false,charging_pause_at:None}],..Default::default()}; assert!(cfg.validate().is_err());
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
    let cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Performance,hz:None,high_touch:false,charging_pause_at:None}],..Default::default()};
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
    let mut cfg = Config { automatic:true, rules:vec![AppRule { package:"com.example.game".into(), preset:Mode::Performance, hz:Some(60),high_touch:false,charging_pause_at:None}], ..Default::default() };
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
    let cfg = Config { automatic:true, rules:vec![AppRule { package:"com.example.game".into(), preset:Mode::Performance, hz:None,high_touch:false,charging_pause_at:None}], ..Default::default() };
    let mut state = RuntimeState::default(); let ctx = context();
    assert_eq!(decide(&cfg, &mut state, &ctx, 100).0, Mode::Boost);
    state.foreground = ctx.app.clone();
    assert_eq!(decide(&cfg, &mut state, &ctx, 101).0, Mode::Boost);
    assert_eq!(state.leases[0].expires, 105);
    assert_eq!(decide(&cfg, &mut state, &ctx, 104).0, Mode::Boost);
    assert_eq!(decide(&cfg, &mut state, &ctx, 105).0, Mode::Performance);
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

#[test]
fn engine_notices_coalesce_and_omit_transient_boosts() {
    use butter::notifications::{Notices,message};
    let mut notices=Notices::default();
    assert!(notices.next("Performance app preset applied.",100));
    assert!(!notices.next("Performance app preset applied.",130));
    assert!(!notices.next("Saver preset active.",110));
    assert!(notices.next("Saver preset active.",116));
    let mut runtime=RuntimeState{active:Mode::Boost,reason:"Temporary boost lease".into(),..Default::default()};
    assert!(message(&runtime).is_none());
    runtime.active=Mode::Performance;runtime.reason="Foreground app rule".into();
    assert_eq!(message(&runtime).unwrap(),"Performance app preset applied.");
    runtime.reason="Foreground app unknown or multiple resumed apps".into();
    assert!(message(&runtime).is_none());
    runtime.error=Some("fixture".into());
    assert_eq!(message(&runtime).unwrap(),"Tuning paused. Open Butterd for details.");
    let config:Config=serde_json::from_str(r#"{"automatic":false,"preset":"balanced","logcat":false,"rules":[]}"#).unwrap();
    assert!(!config.notifications);
}

#[test]
fn notification_confirmation_requires_a_posted_record() {
    use butter::notifications::confirm;
    assert!(confirm("posting:\n Notification(...)\nwaiting for notification to post...\nposted: \n NotificationRecord(...)").is_ok());
    assert!(confirm("posting:\n Notification(...)").is_err());
    assert!(confirm("posting:\nwarning: couldn't find notification after enqueueing").is_err());
    assert!(confirm("Permission Denial:").is_err());
}

#[test]
fn cooling_steps_down_performance_and_cancels_boost_without_renewing_it() {
    let cfg=Config::default(); let mut state=state(); let mut ctx=context();
    state.foreground=ctx.app.clone();
    state.leases.push(Lease{source:"manual".into(),expires:120});
    ctx.temperature=Some(400);
    assert_eq!(decide(&cfg,&mut state,&ctx,100),(Mode::Performance,"Cooling: reduced performance floors"));
    assert_eq!(state.thermal_stage,1); assert!(state.leases.is_empty());
    ctx.temperature=Some(410);
    assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Performance);
    assert_eq!(state.thermal_stage,2);
    ctx.temperature=Some(407); decide(&cfg,&mut state,&ctx,102); assert_eq!(state.thermal_stage,2);
    ctx.temperature=Some(420); assert_eq!(decide(&cfg,&mut state,&ctx,103).0,Mode::Balanced);
    ctx.temperature=Some(400); assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Balanced);
    ctx.temperature=Some(385); assert_eq!(decide(&cfg,&mut state,&ctx,105).0,Mode::Performance);
    assert_eq!(state.thermal_stage,0);
}
#[test]
fn battery_caps_survive_heat_and_foreground_detection_gaps() {
    let cfg=Config{preset:Mode::Battery,..Default::default()};
    let mut state=RuntimeState{tuning_requested:true,selected:Mode::Battery,pending_preset:Some(Mode::Battery),..Default::default()};
    let mut ctx=context(); ctx.temperature=Some(425);
    assert_eq!(decide(&cfg,&mut state,&ctx,100),(Mode::Battery,"Cooling down"));
    ctx.temperature=Some(400); assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Battery);
    ctx.temperature=Some(350); ctx.app=None;
    assert_eq!(decide(&cfg,&mut state,&ctx,102).0,Mode::Battery);
    ctx.temperature=None; assert_eq!(decide(&cfg,&mut state,&ctx,103).0,Mode::Balanced);
}
#[test]
fn soc_cooling_bands_and_recovery_are_independent_of_battery_heat() {
    use butter::runtime::cooling_stage;
    let mut ctx=context();
    for (previous,soc,expected) in [(0,75000,1),(1,74000,1),(1,71000,0),(0,80000,2),(2,79000,2),(2,76000,1),(0,85000,3),(3,75000,3),(3,74000,1)] {
        ctx.soc_millic=Some(soc); assert_eq!(cooling_stage(previous,&ctx),expected);
    }
}
#[test]
fn per_app_battery_retains_caps_but_performance_still_releases_at_cutoff() {
    let mut cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Battery,hz:None,high_touch:false,charging_pause_at:None}],..Default::default()};
    let mut state=RuntimeState::default(); let mut ctx=context(); ctx.soc_millic=Some(86000);
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Battery);
    cfg.rules[0].preset=Mode::Performance;
    assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Balanced);
}

#[test]
fn screen_off_releases_preset_without_forgetting_saved_selection() {
    let cfg=Config{preset:Mode::Battery,..Default::default()};
    let mut state=RuntimeState{tuning_requested:true,selected:Mode::Battery,..Default::default()};
    let mut ctx=context(); ctx.awake=Some(false);
    assert_eq!(decide(&cfg,&mut state,&ctx,100),(Mode::Balanced,"Screen off"));
    assert!(!state.blocked); assert_eq!(state.selected,Mode::Battery);
    state.reason="Screen off".into();
    assert!(butter::notifications::message(&state).is_none());
    ctx.awake=Some(true);
    assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Battery);
}
#[test]
fn recording_parser_distinguishes_capture_mirroring_and_unknown() {
    use butter::recording::parse;
    let header="MEDIA PROJECTION MANAGER (dumpsys media_projection)\nMedia Projection: \n";
    let capture=parse(&format!("{header}(com.example.recorder, uid=10123): TYPE_SCREEN_CAPTURE\n")).unwrap();
    assert_eq!(capture["screenCapture"],true); assert_eq!(capture["package"],"com.example.recorder");
    assert_eq!(parse(&format!("{header}null\n")).unwrap()["active"],false);
    assert_eq!(parse(&format!("{header}(com.example.recorder, uid=10123): TYPE_MIRRORING\n")).unwrap()["screenCapture"],false);
    for text in ["Permission Denial".to_string(),format!("{header}(invalid;name, uid=1): TYPE_SCREEN_CAPTURE"),format!("{header}(com.example.recorder, uid=1): NEW_UNKNOWN_TYPE")] {
        assert!(parse(&text).is_err());
    }
}

#[test]
fn recording_parser_accepts_multiline_rom_sessions_and_rejects_ambiguous_fields() {
    use butter::recording::parse;
    let dump="MEDIA PROJECTION MANAGER (dumpsys media_projection)\r\nMedia Projection: \r\nProjection:\r\n    packageName: com.android.systemui\r\n    uid: 10169\r\n    userHandle: UserHandle{0}\r\n    type: TYPE_SCREEN_CAPTURE\r\n    privileged: true\r\n    mSession: ContentRecordingSession { virtualDisplayId = 3 }\r\n";
    let capture=parse(dump).unwrap();
    assert_eq!(capture["active"],true);
    assert_eq!(capture["screenCapture"],true);
    assert_eq!(capture["package"],"com.android.systemui");
    assert_eq!(parse(&dump.replace("TYPE_SCREEN_CAPTURE","TYPE_MIRRORING")).unwrap()["screenCapture"],false);
    for invalid in [format!("{dump}    type: TYPE_MIRRORING\n"),dump.replace("uid: 10169","uid: invalid"),dump.replace("    type: TYPE_SCREEN_CAPTURE\r\n","")] {
        assert!(parse(&invalid).is_err());
    }
}

#[test]
fn recording_assist_raises_floors_only_with_fresh_capture_and_explicit_opt_in() {
    let mut cfg=Config::default(); let mut state=RuntimeState{tuning_requested:true,..Default::default()}; let ctx=context();
    state.foreground=ctx.app.clone();
    state.telemetry=serde_json::json!({"recording":{"available":true,"screenCapture":true,"sampledAtUptime":100}});
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Balanced);
    cfg.recording_assist=true;
    assert_eq!(decide(&cfg,&mut state,&ctx,101),(Mode::Performance,"Recording assist: capture session active"));
    state.selected=Mode::Performance;
    assert_eq!(decide(&cfg,&mut state,&ctx,102).0,Mode::Boost);
    state.selected=Mode::Battery;
    assert_eq!(decide(&cfg,&mut state,&ctx,103).0,Mode::Battery);
    state.selected=Mode::Balanced;
    assert_eq!(decide(&cfg,&mut state,&ctx,121).0,Mode::Balanced);
    state.telemetry["recording"]["sampledAtUptime"]=serde_json::json!(122);
    state.telemetry["recording"]["screenCapture"]=serde_json::json!(false);
    assert_eq!(decide(&cfg,&mut state,&ctx,122).0,Mode::Balanced);
    state.telemetry["recording"]["screenCapture"]=serde_json::json!(true);
    state.telemetry["recording"]["available"]=serde_json::json!(false);
    assert_eq!(decide(&cfg,&mut state,&ctx,122).0,Mode::Balanced);
}
#[test]
fn recording_assist_reduces_with_heat_and_never_overrides_saver_or_screen_off() {
    let cfg=Config{recording_assist:true,..Default::default()}; let mut state=state(); let mut ctx=context();
    state.foreground=ctx.app.clone();
    state.telemetry=serde_json::json!({"recording":{"available":true,"screenCapture":true,"sampledAtUptime":100}});
    ctx.temperature=Some(400);
    assert_eq!(decide(&cfg,&mut state,&ctx,100),(Mode::Performance,"Recording assist: reduced floors for heat"));
    assert_eq!(state.thermal_stage,1);
    ctx.temperature=Some(420); assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Balanced);
    ctx.temperature=Some(350);ctx.saver=Some(true);assert_eq!(decide(&cfg,&mut state,&ctx,102).0,Mode::Battery);
    ctx.saver=Some(false);ctx.battery=Some(15);assert_eq!(decide(&cfg,&mut state,&ctx,103).0,Mode::Battery);
    ctx.battery=Some(70);ctx.awake=Some(false);assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Balanced);
}

#[test]
fn recording_cutoff_extends_only_eligible_capture_and_retains_step_down() {
    let mut cfg=Config{recording_assist:true,..Default::default()}; let mut state=state(); let mut ctx=context();
    state.foreground=ctx.app.clone();
    state.telemetry=serde_json::json!({"recording":{"available":true,"screenCapture":true,"sampledAtUptime":100}});
    for (now,soc,stage) in [(100,75000,1),(101,80000,2),(102,85000,2),(103,89999,2)] {
        ctx.soc_millic=Some(soc);
        assert_eq!(decide(&cfg,&mut state,&ctx,now).0,Mode::Performance);
        assert_eq!(state.thermal_stage,stage);
    }
    ctx.soc_millic=Some(90000);
    assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Balanced);
    ctx.soc_millic=Some(80000);
    assert_eq!(decide(&cfg,&mut state,&ctx,105).0,Mode::Balanced);
    ctx.soc_millic=Some(70000);
    assert_eq!(decide(&cfg,&mut state,&ctx,106).0,Mode::Boost);
    // Removing consent while recording returns to the ordinary cutoff.
    cfg.recording_assist=false;ctx.soc_millic=Some(86000);
    assert_eq!(decide(&cfg,&mut state,&ctx,107).0,Mode::Balanced);
}
#[test]
fn recording_cutoff_never_relaxes_battery_or_uses_stale_capture() {
    let cfg=Config{recording_assist:true,..Default::default()}; let mut state=state(); let mut ctx=context();
    state.telemetry=serde_json::json!({"recording":{"available":true,"screenCapture":true,"sampledAtUptime":100}});
    ctx.temperature=Some(420);ctx.soc_millic=Some(70000);
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Balanced);
    state.thermal_stage=0;ctx.temperature=Some(350);ctx.soc_millic=Some(86000);
    assert_eq!(decide(&cfg,&mut state,&ctx,121).0,Mode::Balanced);
    state.thermal_stage=0;state.telemetry["recording"]["sampledAtUptime"]=serde_json::json!(122);
    state.telemetry["recording"]["screenCapture"]=serde_json::json!(false);
    assert_eq!(decide(&cfg,&mut state,&ctx,122).0,Mode::Balanced);
}

#[test]
fn aggressive_cools_with_hysteresis_and_obeys_saver_and_screen_off() {
    let cfg=Config::default();
    let mut state=RuntimeState{selected:Mode::Aggressive,tuning_requested:true,..Default::default()};let mut ctx=context();
    state.foreground=ctx.app.clone();
    for (now,soc,stage,mode) in [(100,70000,0,Mode::Aggressive),(101,75000,1,Mode::Aggressive),(102,80000,2,Mode::Aggressive),(103,85000,3,Mode::Balanced),(104,77000,3,Mode::Balanced),(105,74000,1,Mode::Aggressive),(106,71000,0,Mode::Aggressive)] {
        ctx.soc_millic=Some(soc);assert_eq!(decide(&cfg,&mut state,&ctx,now).0,mode);assert_eq!(state.thermal_stage,stage);
    }
    ctx.saver=Some(true);assert_eq!(decide(&cfg,&mut state,&ctx,107).0,Mode::Battery);
    ctx.saver=Some(false);ctx.temperature=Some(425);assert_eq!(decide(&cfg,&mut state,&ctx,108).0,Mode::Balanced);
    ctx.temperature=Some(350);ctx.awake=Some(false);assert_eq!(decide(&cfg,&mut state,&ctx,109).0,Mode::Balanced);
}
#[test]
fn aggressive_app_rule_is_scoped_and_configuration_requires_confirmation() {
    use butter::runtime::acknowledged_args;
    for command in ["preset aggressive", "app com.example.game aggressive 120 high 80"] {
        let args:Vec<String>=command.split_whitespace().map(str::to_string).collect();
        assert!(acknowledged_args(&args).is_err());
        let mut accepted=args.clone();accepted.push("--accept-risk".into());
        assert_eq!(acknowledged_args(&accepted).unwrap(),args.as_slice());
    }
    let cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Aggressive,hz:None,high_touch:false,charging_pause_at:None}],..Default::default()};
    cfg.validate().unwrap();
    let mut state=RuntimeState::default();let mut ctx=context();
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Aggressive);
    state.foreground=ctx.app.clone();ctx.app=Some("com.example.other".into());
    assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Balanced);
}

#[test]
fn aggressive_app_rule_respects_previous_cooling_state() {
    let cfg=Config{automatic:true,rules:vec![AppRule{package:"com.example.game".into(),preset:Mode::Aggressive,hz:None,high_touch:false,charging_pause_at:None}],..Default::default()};
    let mut state=RuntimeState{thermal_stage:3,reason:"Cooling down".into(),..Default::default()};let mut ctx=context();
    ctx.temperature=Some(414);ctx.soc_millic=Some(90000);
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Balanced);
    ctx.app=Some("com.example.other".into());
    assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Balanced);
    state.blocked=true;ctx.app=Some("com.example.game".into());
    assert_eq!(decide(&cfg,&mut state,&ctx,102).0,Mode::Balanced);
}

#[test]
fn force_stop_requires_confirmation_and_bounded_valid_packages() {
    use butter::app_activity::arguments;
    assert_eq!(arguments(&[]).unwrap(),vec!["list"]);
    for command in ["stop com.example.app","stop --confirm com.example.app;reboot","stop --confirm"] {
        let args=command.split_whitespace().map(str::to_string).collect::<Vec<_>>();assert!(arguments(&args).is_err());
    }
    let mut args=vec!["stop".into(),"--confirm".into()];
    args.extend((0..16).map(|n|format!("com.example.app{n}")));assert!(arguments(&args).is_ok());
    args.push("com.example.toomany".into());assert!(arguments(&args).is_err());
}
#[test]
fn battery_history_reports_window_and_rates_without_faking_short_samples() {
    use butter::battery_usage::{parse,duration};
    let text="Statistics since last charge:\nTime on battery: 7h 3m 35s 938ms (98%) realtime, 2h 0m 41s 175ms (28%) uptime\nTime on battery screen off: 5h 23m 51s 490ms (76%) realtime, 20m 56s 727ms (4%) uptime\nStart clock time: 2026-10-24-03-34-18\nDischarge: 1090 mAh\nScreen off discharge: 142 mAh\nScreen on discharge: 948 mAh\nAmount discharged while screen on: 26\nAmount discharged while screen off: 4\n";
    let report=parse(text).unwrap();
    assert!(report["sleepEstimateSeconds"].as_f64().unwrap()>18000.0);
    assert!((report["screenOffMahPerHour"].as_f64().unwrap()-26.3).abs()<0.1);
    assert!(report["screenOnPercentPerHour"].as_f64().unwrap()>15.0);
    let short=text.replace("7h 3m 35s 938ms","10m 0s").replace("2h 0m 41s 175ms","5m 0s").replace("5h 23m 51s 490ms","6m 0s").replace("20m 56s 727ms","1m 0s");
    assert!(parse(&short).unwrap()["screenOffMahPerHour"].is_null());
    assert!(duration("-7h").is_none());assert!(parse("Permission Denial").is_err());
}

#[test]
fn mediatek_gpu_floor_indices_use_descending_full_hardware_steps() {
    use butter::gpu::{floor_index,opp_table,ged_current};
    assert_eq!(floor_index(Mode::Aggressive,0,65).unwrap(),16);
    assert_eq!(floor_index(Mode::Aggressive,2,65).unwrap(),48);
    assert_eq!(floor_index(Mode::Performance,0,65).unwrap(),32);
    assert_eq!(floor_index(Mode::Performance,1,65).unwrap(),43);
    assert_eq!(floor_index(Mode::Performance,2,65).unwrap(),54);
    assert_eq!(floor_index(Mode::Battery,0,65).unwrap(),64);
    assert_eq!(ged_current("64 265000"),Some((64,265000000)));
    assert!(ged_current("64 -1").is_none());
    assert!(opp_table("[00] freq: 900000, volt: 1\n[02] freq: 100000, volt: 1","900000000 100000000",2).is_err());
    assert!(opp_table("[00] freq: 100000, volt: 1\n[01] freq: 900000, volt: 1","900000000 100000000",2).is_err());
}

#[test]
fn aggressive_recording_keeps_selected_floors_and_idle_releases_them() {
    let cfg=Config{recording_assist:true,..Default::default()};
    let mut state=RuntimeState{selected:Mode::Aggressive,tuning_requested:true,..Default::default()};let mut ctx=context();
    state.foreground=ctx.app.clone();
    state.telemetry=serde_json::json!({"cpuBusyPercent":50.0,"recording":{"available":true,"screenCapture":true,"sampledAtUptime":100}});
    ctx.soc_millic=Some(87000);
    assert_eq!(decide(&cfg,&mut state,&ctx,100).0,Mode::Aggressive);
    assert_eq!(state.thermal_stage,2);
    ctx.soc_millic=Some(91000);assert_eq!(decide(&cfg,&mut state,&ctx,101).0,Mode::Balanced);
    ctx.soc_millic=Some(45000);state.telemetry["cpuBusyPercent"]=serde_json::json!(0.0);
    for now in 102..104 {assert_eq!(decide(&cfg,&mut state,&ctx,now).0,Mode::Aggressive);}
    assert_eq!(decide(&cfg,&mut state,&ctx,104).0,Mode::Balanced);
    state.telemetry["cpuBusyPercent"]=serde_json::json!(60.0);
    assert_eq!(decide(&cfg,&mut state,&ctx,105).0,Mode::Aggressive);
}

#[test]
fn charging_pause_and_bypass_ignore_butter_temperature_gates_only() {
    use butter::{model::Feature,runtime::override_temperature_allowed};
    let mut ctx=context();ctx.temperature=Some(455);ctx.soc_millic=Some(91000);
    for starting in [true,false] {
        for feature in [Feature::ChargingPause,Feature::Bypass] {assert!(override_temperature_allowed(&feature,&ctx,starting));}
        for feature in [Feature::ChargingThermal,Feature::CpuLock,Feature::GpuLock,Feature::GpuThermal] {assert!(!override_temperature_allowed(&feature,&ctx,starting));}
    }
    ctx.temperature=None;ctx.soc_millic=None;
    assert!(override_temperature_allowed(&Feature::ChargingPause,&ctx,true));
    assert!(!override_temperature_allowed(&Feature::ChargingThermal,&ctx,true));
}

#[test]
fn notification_updates_ignore_wake_and_sampling_but_report_charging_changes() {
    use butter::notifications::Notices;
    let mut notices=Notices::default();
    let mut runtime=RuntimeState{active:Mode::Battery,reason:"Selected preset".into(),..Default::default()};
    assert_eq!(notices.update(&runtime,100).as_deref(),Some("Saver preset active."));
    runtime.active=Mode::Balanced;runtime.reason="Screen off".into();
    assert!(notices.update(&runtime,120).is_none());
    runtime.active=Mode::Battery;runtime.reason="Selected preset".into();
    assert!(notices.update(&runtime,140).is_none());
    runtime.reason="Foreground app unknown or multiple resumed apps".into();
    assert!(notices.update(&runtime,160).is_none());
    runtime.reason="Selected preset".into();assert!(notices.update(&runtime,180).is_none());
    runtime.app_controls.charging_owned=true;
    assert_eq!(notices.update(&runtime,200).as_deref(),Some("Bypass charging active."));
    assert!(notices.update(&runtime,220).is_none());
    runtime.app_controls.charging_owned=false;
    assert_eq!(notices.update(&runtime,240).as_deref(),Some("Bypass charging stopped."));
    runtime.active=Mode::Performance;runtime.reason="Foreground app rule".into();
    assert_eq!(notices.update(&runtime,260).as_deref(),Some("Performance app preset applied."));
    runtime.error=Some("fixture".into());
    assert_eq!(notices.update(&runtime,280).as_deref(),Some("Tuning paused. Open Butterd for details."));
    assert!(notices.update(&runtime,300).is_none());
}

#[test]
fn aggressive_cooling_reasons_name_the_actual_preset() {
    let cfg=Config::default();let mut state=RuntimeState{selected:Mode::Aggressive,tuning_requested:true,..Default::default()};let mut ctx=context();
    ctx.soc_millic=Some(75000);
    assert_eq!(decide(&cfg,&mut state,&ctx,100),(Mode::Aggressive,"Cooling: Aggressive clock floors reduced"));
    ctx.soc_millic=Some(80000);
    assert_eq!(decide(&cfg,&mut state,&ctx,101),(Mode::Aggressive,"Cooling: Aggressive clock floors reduced further"));
}
