use goxlr_ipc::DaemonStatus;
use goxlr_types::GenderStyle;
use goxlr_ui::app::Msg;
use goxlr_ui::{AppWindow, table, wire, with_app};
use slint::PlatformError;
use slint::platform::software_renderer::{MinimalSoftwareWindow, RepaintBufferType};
use slint::platform::{Platform, WindowAdapter};
use std::rc::Rc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedReceiver;

struct P(Rc<MinimalSoftwareWindow>, Instant);

impl Platform for P {
    fn create_window_adapter(&self) -> Result<Rc<dyn WindowAdapter>, PlatformError> {
        Ok(self.0.clone())
    }
    fn duration_since_start(&self) -> Duration {
        self.1.elapsed()
    }
}

fn status() -> DaemonStatus {
    serde_json::from_str(include_str!("mock.json")).expect("mock status")
}

fn drain(rx: &mut UnboundedReceiver<Msg>) -> Vec<String> {
    let mut out = vec![];
    while let Ok(m) = rx.try_recv() {
        match m {
            Msg::Mic(_) => {}
            other => out.push(format!("{other:?}")),
        }
    }
    out
}

fn press(rx: &mut UnboundedReceiver<Msg>, key: &str, value: f32, text: &str) -> Vec<String> {
    with_app(|a| a.event(key, value, text));
    drain(rx)
}

#[test]
fn events_become_the_right_daemon_commands() {
    let window = MinimalSoftwareWindow::new(RepaintBufferType::NewBuffer);
    slint::platform::set_platform(Box::new(P(window, Instant::now()))).unwrap();
    let ui = AppWindow::new().unwrap();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    wire(&ui, tx);
    with_app(|a| a.on_status(Some(status())));
    drain(&mut rx);

    let cases: &[(&str, f32, &str, &str)] = &[
        (
            "mic.gain",
            55.0,
            "",
            "Cmd(\"S12345\", SetMicrophoneGain(Dynamic, 55))",
        ),
        ("gate.on", 1.0, "", "SetGateActive(true)"),
        ("gate.atk", 3.0, "", "SetGateAttack(Gate40ms)"),
        ("comp.makeup", -3.0, "", "SetCompressorMakeupGain(-3)"),
        ("fx.rev.amount", 40.0, "", "SetReverbAmount(40)"),
        ("fx.robot.lg", -3.0, "", "SetRobotGain(Low, -3)"),
        ("fx.echo.dl", 2400.0, "", "SetEchoDelayLeft(2400)"),
        ("mix.cough", 1.0, "", "SetCoughMuteState(MutedToX)"),
        ("mix.fch.A", 2.0, "", "SetFader(A, Console)"),
        ("scr.top.A", 0.0, "hi", "SetScribbleNumber(A, \"hi\")"),
        ("sub.vol.2", 77.0, "", "SetSubMixVolume(Console, 77)"),
        ("sub.link.2", 1.0, "", "SetSubMixLinked(Console, true)"),
        ("eq.gain.3", 4.0, "", "SetEqGain(Equalizer250Hz, 4)"),
        ("eq.freq.3", 0.0, "250", "SetEqFreq(Equalizer250Hz, 250.0)"),
        (
            "samp.mode.A.TopLeft",
            5.0,
            "",
            "SetSamplerFunction(A, TopLeft, Loop)",
        ),
        (
            "samp.s.A.TopLeft.0|0",
            0.0,
            "",
            "PlaySampleByIndex(A, TopLeft, 0)",
        ),
        ("prof.2|0", 0.0, "", "LoadProfile(\"Gaming\", true)"),
        ("prof.saveas", 0.0, "NewName", "SaveProfileAs(\"NewName\")"),
        ("path.0|0", 0.0, "", "OpenPath(Profiles)"),
        ("d.tray", 0.0, "", "SetShowTrayIcon(false)"),
        ("d.log", 4.0, "", "SetLogLevel(Debug)"),
        ("d.locale", 0.0, "de", "SetLocale(Some(\"de\"))"),
        ("d.locale", 0.0, "  ", "SetLocale(None)"),
        (
            "set.disp.1",
            1.0,
            "",
            "SetElementDisplayMode(Equaliser, Advanced)",
        ),
        ("fx.load", 1.0, "", "LoadEffectPreset(\"Cave\")"),
    ];
    for (key, value, text, expect) in cases {
        let got = press(&mut rx, key, *value, text);
        assert!(
            got.iter().any(|g| g.contains(expect)),
            "{key}: expected {expect}, got {got:?}"
        );
    }

    // Destructive actions only run on the second press.
    let confirmed: &[(&str, &str)] = &[
        ("samp.s.A.TopLeft.0|1", "RemoveSampleByIndex(A, TopLeft, 0)"),
        ("fw.update", "RunFirmwareUpdate(\"S12345\", None, false)"),
        ("d.stop", "StopDaemon"),
    ];
    for (key, expect) in confirmed {
        assert!(
            press(&mut rx, key, 0.0, "").is_empty(),
            "{key} ran on the first press"
        );
        let got = press(&mut rx, key, 0.0, "");
        assert!(
            got.iter().any(|g| g.contains(expect)),
            "{key}: expected {expect}, got {got:?}"
        );
    }

    // A confirmation is dropped if anything else is pressed in between.
    press(&mut rx, "d.stop", 0.0, "");
    press(&mut rx, "d.tray", 1.0, "");
    drain(&mut rx);
    assert!(press(&mut rx, "d.stop", 0.0, "").is_empty());

    // Colour edits need the lighting page so the other colours of the row are known.
    ui.set_page(5);
    ui.set_light_tab(1);
    with_app(|a| a.refresh());
    drain(&mut rx);
    let got = press(&mut rx, "light.fader.A:1", 0xFF8800 as f32, "");
    assert!(
        got.iter()
            .any(|g| g.contains("SetFaderColours(A, \"00FFFF\", \"FF8800\")")),
        "{got:?}"
    );

    // Unknown keys do nothing.
    assert!(press(&mut rx, "nope.nope", 1.0, "").is_empty());
}

#[test]
fn slider_ranges_follow_the_effect_style() {
    let mut st = status();
    let mixer = st.mixers.get_mut("S12345").unwrap();
    let max_of = |m: &goxlr_ipc::MixerStatus| {
        let rows = table::build(3, 4, m);
        rows.iter()
            .find(|c| c.key == "fx.gender.amount")
            .map(|c| (c.min, c.max))
            .unwrap()
    };
    mixer.effects.as_mut().unwrap().current.gender.style = GenderStyle::Narrow;
    assert_eq!(max_of(mixer), (-12.0, 12.0));
    mixer.effects.as_mut().unwrap().current.gender.style = GenderStyle::Medium;
    assert_eq!(max_of(mixer), (-25.0, 25.0));
    mixer.effects.as_mut().unwrap().current.gender.style = GenderStyle::Wide;
    assert_eq!(max_of(mixer), (-50.0, 50.0));
}

#[test]
fn firmware_tab_follows_the_update_state() {
    use goxlr_ipc::{FirmwareInfo, FirmwareStatus, UpdateState};
    use goxlr_types::{DeviceType, VersionNumber};
    use goxlr_ui::pages;

    let mut st = status();
    let keys = |st: &DaemonStatus| -> Vec<String> {
        let m = st.mixers.get("S12345").unwrap();
        pages::settings(st, Some(m), None, 2, "")
            .into_iter()
            .map(|c| c.key)
            .collect()
    };
    let set = |st: &mut DaemonStatus, state: UpdateState, error: Option<&str>| {
        st.firmware.insert(
            "S12345".into(),
            FirmwareStatus {
                state,
                progress: 40,
                error: error.map(String::from),
            },
        );
    };

    assert!(keys(&st).contains(&"fw.update".to_string()));

    set(&mut st, UpdateState::Download, None);
    let busy = keys(&st);
    assert!(
        !busy.iter().any(|k| k.starts_with("fw.")),
        "no buttons while an update runs: {busy:?}"
    );

    set(&mut st, UpdateState::Failed, Some("boom"));
    let failed = keys(&st);
    assert!(failed.contains(&"fw.clear".to_string()) && !failed.contains(&"fw.update".to_string()));

    let info = FirmwareInfo {
        path: "x.bin".into(),
        device_type: DeviceType::Full,
        version: VersionNumber(1, 0, Some(0), Some(0)),
    };
    set(&mut st, UpdateState::Pause(info), None);
    let paused = keys(&st);
    assert!(
        paused.contains(&"fw.continue".to_string()) && paused.contains(&"fw.clear".to_string())
    );
}
