use crate::model::{C, hex_to_rgb, idx, opts, pretty};
use crate::table;
use goxlr_ipc::{DaemonStatus, MixerStatus, PathTypes};
use goxlr_types::*;
use strum::IntoEnumIterator;

pub const PATHS: [(PathTypes, &str); 7] = [
    (PathTypes::Profiles, "Profiles"),
    (PathTypes::MicProfiles, "Mic profiles"),
    (PathTypes::Presets, "Effect presets"),
    (PathTypes::Samples, "Samples"),
    (PathTypes::Icons, "Icons"),
    (PathTypes::Logs, "Logs"),
    (PathTypes::Backups, "Backups"),
];
pub const LOG_LEVELS: [&str; 6] = ["Off", "Error", "Warn", "Info", "Debug", "Trace"];

pub struct Ctx<'a> {
    pub st: &'a DaemonStatus,
    pub m: &'a MixerStatus,
    pub confirm: Option<&'a str>,
    pub adv: bool,
}

pub fn tabs(page: u8, adv: bool) -> Vec<&'static str> {
    match (page, adv) {
        (2, true) => vec!["Input", "Gate", "Compressor", "Equaliser"],
        (3, true) => vec!["Presets", "Reverb", "Echo", "Pitch", "Gender", "Megaphone", "Robot", "Hard tune"],
        (4, true) => vec!["Bank A", "Bank B", "Bank C", "Files"],
        (4, false) => vec!["Bank A", "Bank B", "Bank C"],
        (5, true) => vec!["Animation", "Faders", "Buttons", "Other"],
        (6, true) => vec!["Profiles", "Mic profiles", "Folders"],
        (6, false) => vec!["Profiles", "Mic profiles"],
        (7, true) => vec!["Mixer", "Daemon", "Device"],
        _ => vec![],
    }
}

fn names<T: IntoEnumIterator + std::fmt::Display>() -> Vec<String> {
    opts::<T>()
}

pub fn mixer(c: &Ctx) -> Vec<C> {
    let m = c.m;
    if !c.adv {
        let inputs: Vec<String> = ChannelName::iter().take(8).map(|c| pretty(&c.to_string())).collect();
        let mut out = vec![
            C::hint("Drag a slider to change how loud something is. Press Mute to silence it. The labels under each name show which channel each of your hardware faders controls."),
            C::heading("Your microphone"),
            C::toggle("mix.cough", "Mute my microphone", m.cough_button.state != MuteState::Unmuted),
            C::heading("What each hardware fader controls"),
        ];
        for f in FaderName::iter() {
            let ci = ChannelName::iter().take(8).position(|x| x == m.fader_status[f].channel).map_or(-1, |i| i as i32);
            out.push(C::choice(&format!("mix.fch.{f}"), &format!("Fader {f}"), inputs.clone(), ci));
        }
        return out;
    }
    let mut out = vec![
        C::heading("Microphone"),
        C::toggle("mix.cough", "Cough button muted", m.cough_button.state != MuteState::Unmuted),
        C::slider("mix.bleep", "Bleep volume", -34.0, 0.0, 1.0, m.levels.bleep as f32, " dB"),
        C::heading("Faders"),
    ];
    let inputs: Vec<String> = ChannelName::iter().take(8).map(|c| pretty(&c.to_string())).collect();
    for f in FaderName::iter() {
        let fs = &m.fader_status[f];
        let ci = ChannelName::iter().take(8).position(|x| x == fs.channel).map_or(-1, |i| i as i32);
        out.push(C::choice(&format!("mix.fch.{f}"), &format!("Fader {f} channel"), inputs.clone(), ci));
        out.push(C::choice(&format!("mix.ffn.{f}"), &format!("Fader {f} mute"), names::<MuteFunction>(), idx(&fs.mute_type)));
    }
    out.push(C::toggle("mix.lock", "Lock faders", m.settings.lock_faders));
    if m.hardware.device_type == DeviceType::Full {
        out.push(C::heading("Scribbles"));
        let mut icon_opts = vec!["None".to_string()];
        icon_opts.extend(c.st.files.icons.iter().cloned());
        for f in FaderName::iter() {
            let s = m.fader_status[f].scribble.clone().unwrap_or_default();
            let ii = s.file_name.as_ref().and_then(|n| c.st.files.icons.iter().position(|i| i == n)).map_or(0, |i| i as i32 + 1);
            out.push(C::choice(&format!("scr.icon.{f}"), &format!("{f} icon"), icon_opts.clone(), ii));
            out.push(C::field(&format!("scr.top.{f}"), &format!("{f} top text"), s.left_text.as_deref().unwrap_or("")));
            out.push(C::field(&format!("scr.bot.{f}"), &format!("{f} bottom text"), s.bottom_text.as_deref().unwrap_or("")));
            out.push(C::toggle(&format!("scr.inv.{f}"), &format!("{f} inverted"), s.inverted));
        }
    }
    out
}

pub fn routing(c: &Ctx) -> Vec<C> {
    let m = c.m;
    let mut out = vec![
        C::heading("Monitor"),
        C::choice("mon.mix", "Monitor mix", names::<OutputDevice>(), idx(&m.levels.output_monitor)),
        C::toggle("mon.fx", "Monitor with effects", m.settings.enable_monitor_with_fx),
    ];
    if m.levels.submix_supported {
        out.push(C::heading("Sub mix"));
        out.push(C::toggle("sub.on", "Enable sub mix", m.levels.submix.is_some()));
        if let Some(sub) = &m.levels.submix {
            for (i, ch) in SubMixChannelName::iter().enumerate() {
                let s = &sub.inputs[ch];
                out.push(C::slider_link(&format!("sub.vol.{i}"), &pretty(&ch.to_string()), 0.0, 255.0, s.volume as f32, &format!("sub.link.{i}"), s.linked));
            }
            out.push(C::heading("Output mix"));
            for (i, o) in OutputDevice::iter().enumerate() {
                out.push(C::seg(&format!("sub.out.{i}"), &pretty(&o.to_string()), vec!["A".into(), "B".into()], idx(&sub.outputs[o])));
            }
        }
    }
    out
}

pub fn matrix(m: &MixerStatus) -> (Vec<String>, Vec<String>, Vec<Vec<i32>>) {
    let inputs: Vec<String> = InputDevice::iter().map(|i| pretty(&i.to_string())).collect();
    let outputs: Vec<String> = OutputDevice::iter().map(|o| pretty(&o.to_string())).collect();
    let rows = InputDevice::iter()
        .map(|i| {
            OutputDevice::iter()
                .map(|o| {
                    if i == InputDevice::Chat && o == OutputDevice::ChatMic {
                        2
                    } else {
                        m.router[i][o] as i32
                    }
                })
                .collect()
        })
        .collect();
    (inputs, outputs, rows)
}

fn tone_groups(m: &MixerStatus) -> Vec<(&'static str, Vec<usize>, f32)> {
    let mini = m.hardware.device_type == DeviceType::Mini;
    let groups: [(&str, Vec<usize>); 3] = if mini {
        [("Bass", vec![0, 1]), ("Mids", vec![2, 3]), ("Treble", vec![4, 5])]
    } else {
        [("Bass", vec![0, 1, 2]), ("Mids", vec![3, 4, 5, 6]), ("Treble", vec![7, 8, 9])]
    };
    groups
        .into_iter()
        .map(|(n, idxs)| {
            let sum: i32 = idxs
                .iter()
                .map(|&i| {
                    if mini {
                        MiniEqFrequencies::iter().nth(i).and_then(|b| m.mic_status.equaliser_mini.gain.get(&b).copied()).unwrap_or(0) as i32
                    } else {
                        EqFrequencies::iter().nth(i).and_then(|b| m.mic_status.equaliser.gain.get(&b).copied()).unwrap_or(0) as i32
                    }
                })
                .sum();
            (n, idxs.clone(), (sum as f32 / idxs.len() as f32).round())
        })
        .collect()
}

pub fn tone_bands(m: &MixerStatus, name: &str) -> Vec<usize> {
    tone_groups(m).into_iter().find(|g| g.0 == name).map(|g| g.1).unwrap_or_default()
}

pub fn mic(c: &Ctx, tab: u8) -> Vec<C> {
    if !c.adv {
        let m = c.m;
        let mut out = vec![
            C::hint("Set up your microphone here. Talk normally and watch the level bar: it should reach the middle or a little higher, without hitting the red end."),
            C::heading("Microphone"),
        ];
        out.extend(table::pick(m, "mic.type", "Microphone type"));
        out.extend(table::pick(m, "mic.gain", "Microphone volume"));
        out.extend(table::pick(m, "mic.deess", "Soften harsh \"S\" sounds"));
        out.push(C::heading("Cut background noise"));
        out.extend(table::pick(m, "gate.on", "Noise gate on"));
        out.extend(table::pick(m, "gate.thr", "How much to cut"));
        out.push(C::heading("Tone"));
        for (name, _, avg) in tone_groups(m) {
            out.push(C::slider(&format!("tone.{name}"), name, -9.0, 9.0, 1.0, avg, " dB"));
        }
        return out;
    }
    if tab != 3 {
        return table::build(table::MIC, tab, c.m);
    }
    let m = c.m;
    let mut out = vec![C::heading("Gain")];
    let mut freqs = vec![C::heading("Centre frequency (Hz)")];
    if m.hardware.device_type == DeviceType::Mini {
        for (i, b) in MiniEqFrequencies::iter().enumerate() {
            let g = m.mic_status.equaliser_mini.gain.get(&b).copied().unwrap_or(0);
            out.push(C::slider(&format!("eq.gain.{i}"), &pretty(&b.to_string()), -9.0, 9.0, 1.0, g as f32, " dB"));
            let f = m.mic_status.equaliser_mini.frequency.get(&b).copied().unwrap_or(0.0);
            freqs.push(C::field(&format!("eq.freq.{i}"), &pretty(&b.to_string()), &format!("{}", f.round())));
        }
    } else {
        for (i, b) in EqFrequencies::iter().enumerate() {
            let g = m.mic_status.equaliser.gain.get(&b).copied().unwrap_or(0);
            out.push(C::slider(&format!("eq.gain.{i}"), &pretty(&b.to_string()), -9.0, 9.0, 1.0, g as f32, " dB"));
            let f = m.mic_status.equaliser.frequency.get(&b).copied().unwrap_or(0.0);
            freqs.push(C::field(&format!("eq.freq.{i}"), &pretty(&b.to_string()), &format!("{}", f.round())));
        }
    }
    out.extend(freqs);
    out
}

pub fn effects(c: &Ctx, tab: u8) -> Vec<C> {
    let Some(e) = &c.m.effects else {
        return vec![C::note("Effects are not available on this device.")];
    };
    if !c.adv {
        let m = c.m;
        let mut out = vec![
            C::hint("Change how your voice sounds. Switch effects on, pick one of the six presets, then fine-tune with the sliders. Use Advanced mode for every detailed setting."),
            C::heading("Presets"),
            C::toggle("fx.on", "Voice effects on", e.is_enabled),
            C::seg("fx.preset", "Preset", (1..=6).map(|i| i.to_string()).collect(), idx(&e.active_preset)),
            C::heading("Quick controls"),
        ];
        for (k, l) in [("fx.rev.amount", "Reverb (room echo)"), ("fx.echo.amount", "Echo"), ("fx.pitch.amount", "Pitch (lower to higher)"), ("fx.gender.amount", "Voice tone (deeper to lighter)")] {
            out.extend(table::pick(m, k, l));
        }
        out.push(C::heading("Voice changers"));
        for (k, l) in [("fx.mega.on", "Megaphone"), ("fx.robot.on", "Robot"), ("fx.hard.on", "Auto-tune")] {
            out.extend(table::pick(m, k, l));
        }
        return out;
    }
    if tab != 0 {
        return table::build(table::FX, tab, c.m);
    }
    let active = e.preset_names.get(&e.active_preset).cloned().unwrap_or_default();
    vec![
        C::heading("Presets"),
        C::toggle("fx.on", "Effects enabled", e.is_enabled),
        C::seg("fx.preset", "Active preset", (1..=6).map(|i| i.to_string()).collect(), idx(&e.active_preset)),
        C::field("fx.rename", "Preset name", &active),
        C::button("fx.save", "Save changes", "Save preset", false),
        C::choice("fx.load", "Load from file", c.st.files.presets.clone(), -1),
    ]
}

pub fn sampler(c: &Ctx, tab: u8) -> Vec<C> {
    let Some(s) = &c.m.sampler else {
        return vec![C::note("Sampler is not available on this device.")];
    };
    if tab == 3 {
        let mut out = vec![
            C::heading("Sampler"),
            C::seg("samp.active", "Active bank", vec!["A".into(), "B".into(), "C".into()], idx(&s.active_bank)),
            C::toggle("samp.reset", "Reset on clear", c.m.settings.reset_sampler_on_clear),
            C::slider("samp.fade", "Fade time", 0.0, 5000.0, 50.0, c.m.settings.fade_duration as f32, " ms"),
            C::slider("samp.buf", "Record buffer", 0.0, 30000.0, 500.0, s.record_buffer as f32, " ms"),
        ];
        if let Some(err) = &s.processing_state.last_error {
            out.push(C::info("Last error", err));
            out.push(C::button("samp.clearerr", "", "Dismiss", false));
        }
        out.push(C::heading("Sample files (gain)"));
        if c.st.files.samples.is_empty() {
            out.push(C::note("No sample files found."));
        }
        for (i, (_, f)) in c.st.files.samples.iter().enumerate() {
            out.push(C::slider(&format!("samp.gain.{i}"), &f.name, 0.0, 100.0, 1.0, f.gain_pct as f32, "%"));
        }
        return out;
    }
    let bank = SampleBank::iter().nth(tab as usize).unwrap_or(SampleBank::A);
    let mut out = vec![];
    if !c.adv {
        out.push(C::hint("Each bank holds sounds for the four sampler buttons on your device. Add a sound to a button, then press the button (or Play here) to hear it."));
    }
    out.push(C::seg("samp.active", "Bank used by the device", vec!["A".into(), "B".into(), "C".into()], idx(&s.active_bank)));
    let Some(btns) = s.banks.get(&bank) else { return out };
    let sample_names: Vec<String> = c.st.files.samples.keys().cloned().collect();
    for b in SampleButtons::iter() {
        let Some(btn) = btns.get(&b) else { continue };
        out.push(C::heading(&format!("{}{}", pretty(&b.to_string()), if btn.is_playing { "  (playing)" } else { "" })));
        if c.adv {
            out.push(C::choice(&format!("samp.mode.{bank}.{b}"), "Mode", names::<SamplePlaybackMode>(), idx(&btn.function)));
            out.push(C::seg(&format!("samp.order.{bank}.{b}"), "Order", names::<SamplePlayOrder>(), idx(&btn.order)));
        }
        for (i, smp) in btn.samples.iter().enumerate() {
            let k = format!("{bank}.{b}.{i}");
            let confirm_key = format!("samp.s.{k}|1");
            out.push(C::item(&format!("samp.s.{k}"), &smp.name, false, true, ["Play", if c.confirm == Some(&confirm_key) { "Confirm?" } else { "Remove" }]));
            if c.adv {
                out.push(C::slider(&format!("samp.start.{k}"), "Start", 0.0, 100.0, 0.1, smp.start_pct, "%"));
                out.push(C::slider(&format!("samp.end.{k}"), "End", 0.0, 100.0, 0.1, smp.stop_pct, "%"));
            }
        }
        out.push(C::choice(&format!("samp.add.{bank}.{b}"), "Add sample", sample_names.clone(), -1));
        out.push(C::button(&format!("samp.halt.{bank}.{b}"), "", "Stop playback", false));
    }
    out
}

fn rgb(s: &str) -> (u8, u8, u8) {
    hex_to_rgb(s)
}

pub fn lighting(c: &Ctx, tab: u8) -> Vec<C> {
    let l = &c.m.lighting;
    let mut out = vec![];
    if !c.adv {
        out.push(C::hint("Pick the colours that light up your device. Click a coloured circle and slide R, G and B to mix your own colour."));
        for (i, t) in SimpleColourTargets::iter().enumerate() {
            let label = match t {
                SimpleColourTargets::Global => "Main colour",
                SimpleColourTargets::Accent => "Accent colour",
                _ => continue,
            };
            if let Some(s) = l.simple.get(&t) {
                out.push(C::colours(&format!("light.simple.{i}"), label, vec![rgb(&s.colour_one)]));
            }
        }
        if let Some(f) = FaderName::iter().find_map(|f| l.faders.get(&f)) {
            out.push(C::colours("light.allf", "Fader colours (top, bottom)", vec![rgb(&f.colours.colour_one), rgb(&f.colours.colour_two)]));
        }
        if l.animation.supported {
            out.push(C::choice("light.anim", "Animation", names::<AnimationMode>(), idx(&l.animation.mode)));
        }
        return out;
    }
    match tab {
        0 => {
            if !l.animation.supported {
                return vec![C::note("Animations are not supported on this device.")];
            }
            out.push(C::choice("light.anim", "Mode", names::<AnimationMode>(), idx(&l.animation.mode)));
            out.push(C::slider("light.mod1", "Modifier 1", 0.0, 255.0, 1.0, l.animation.mod1 as f32, ""));
            out.push(C::slider("light.mod2", "Modifier 2", 0.0, 255.0, 1.0, l.animation.mod2 as f32, ""));
            out.push(C::seg("light.water", "Waterfall", names::<WaterfallDirection>(), idx(&l.animation.waterfall_direction)));
        }
        1 => {
            let styles = names::<FaderDisplayStyle>();
            out.push(C::heading("All faders"));
            let first = FaderName::iter().find_map(|f| l.faders.get(&f));
            if let Some(f) = first {
                out.push(C::colours("light.allf", "Colours", vec![rgb(&f.colours.colour_one), rgb(&f.colours.colour_two)]));
            }
            out.push(C::choice("light.allstyle", "Style", styles.clone(), -1));
            for f in FaderName::iter() {
                let Some(fl) = l.faders.get(&f) else { continue };
                out.push(C::heading(&format!("Fader {f}")));
                out.push(C::colours(&format!("light.fader.{f}"), "Colours", vec![rgb(&fl.colours.colour_one), rgb(&fl.colours.colour_two)]));
                out.push(C::choice(&format!("light.fstyle.{f}"), "Style", styles.clone(), idx(&fl.style)));
            }
        }
        2 => {
            for (i, b) in Button::iter().enumerate() {
                let Some(bl) = l.buttons.get(&b) else { continue };
                out.push(C::heading(&pretty(&b.to_string())));
                out.push(C::colours(&format!("light.btn.{i}"), "On / off colour", vec![rgb(&bl.colours.colour_one), rgb(&bl.colours.colour_two)]));
                out.push(C::choice(&format!("light.boff.{i}"), "When off", names::<ButtonColourOffStyle>(), idx(&bl.off_style)));
            }
        }
        _ => {
            out.push(C::heading("General"));
            for (i, t) in SimpleColourTargets::iter().enumerate() {
                if let Some(s) = l.simple.get(&t) {
                    out.push(C::colours(&format!("light.simple.{i}"), &pretty(&t.to_string()), vec![rgb(&s.colour_one)]));
                }
            }
            if !l.encoders.is_empty() {
                out.push(C::heading("Encoders"));
            }
            for (i, t) in EncoderColourTargets::iter().enumerate() {
                if let Some(e) = l.encoders.get(&t) {
                    out.push(C::colours(&format!("light.enc.{i}"), &pretty(&t.to_string()), vec![rgb(&e.colour_one), rgb(&e.colour_two), rgb(&e.colour_three)]));
                }
            }
            if !l.sampler.is_empty() {
                out.push(C::heading("Sampler banks"));
            }
            for (i, t) in SamplerColourTargets::iter().enumerate() {
                if let Some(s) = l.sampler.get(&t) {
                    out.push(C::colours(&format!("light.samp.{i}"), &pretty(&t.to_string()), vec![rgb(&s.colours.colour_one), rgb(&s.colours.colour_two), rgb(&s.colours.colour_three)]));
                    out.push(C::choice(&format!("light.soff.{i}"), "When off", names::<ButtonColourOffStyle>(), idx(&s.off_style)));
                }
            }
        }
    }
    out
}

pub fn profiles(c: &Ctx, tab: u8) -> Vec<C> {
    let (prefix, files, current, kind): (&str, &Vec<String>, &str, &str) = match tab {
        0 => ("prof", &c.st.files.profiles, &c.m.profile_name, "profile"),
        1 => ("mprof", &c.st.files.mic_profiles, &c.m.mic_profile_name, "mic profile"),
        _ => {
            let mut out = vec![];
            for (i, (_, name)) in PATHS.iter().enumerate() {
                let k = format!("path.{i}");
                let ck = format!("{k}|1");
                out.push(C::item(&k, name, false, true, ["Open", if c.confirm == Some(&ck) { "Confirm restore?" } else { "Restore defaults" }]));
            }
            return out;
        }
    };
    let mut out = vec![C::heading(if tab == 0 { "Profiles" } else { "Mic profiles" })];
    for (i, n) in files.iter().enumerate() {
        let k = format!("{prefix}.{i}");
        let ck = format!("{k}|1");
        out.push(C::item(&k, n, n == current, n != current, ["Load", if c.confirm == Some(&ck) { "Confirm delete?" } else { "Delete" }]));
    }
    out.push(C::heading("Manage"));
    out.push(C::button(&format!("{prefix}.save"), &format!("Save changes to this {kind}"), "Save", false));
    out.push(C::field(&format!("{prefix}.saveas"), "Save as a copy", ""));
    out.push(C::field(&format!("{prefix}.new"), "Create a new one", ""));
    out
}

pub fn settings(st: &DaemonStatus, m: Option<&MixerStatus>, confirm: Option<&str>, tab: u8, adv: bool) -> Vec<C> {
    let cfg = &st.config;
    if !adv {
        let mut out = vec![
            C::hint("Everyday settings. Switch on Advanced mode (bottom left) for network, logging, firmware and hardware options."),
            C::heading("Application"),
            C::toggle("d.auto", "Start with my computer", cfg.autostart_enabled),
            C::toggle("d.tray", "Show tray icon", cfg.show_tray_icon),
        ];
        if let Some(m) = m {
            out.push(C::heading("About your device"));
            out.push(C::info("Model", &format!("{:?}", m.hardware.device_type)));
            out.push(C::info("Serial number", &m.hardware.serial_number));
            out.push(C::info("Firmware", &m.hardware.versions.firmware.to_string()));
        }
        out.push(C::info("Utility version", &cfg.daemon_version));
        return out;
    }
    match tab {
        0 => {
            let Some(m) = m else { return vec![C::note("No GoXLR detected.")] };
            let mut out = table::build(table::SETTINGS, 0, m);
            out.push(C::heading("Hardware display mode"));
            for (i, comp) in DisplayModeComponents::iter().enumerate() {
                let d = &m.settings.display;
                let cur = match comp {
                    DisplayModeComponents::NoiseGate => d.gate,
                    DisplayModeComponents::Equaliser => d.equaliser,
                    DisplayModeComponents::Compressor => d.compressor,
                    DisplayModeComponents::EqFineTune => d.equaliser_fine,
                };
                out.push(C::seg(&format!("set.disp.{i}"), &pretty(&comp.to_string()), names::<DisplayMode>(), idx(&cur)));
            }
            out
        }
        1 => {
            let mut out = vec![
                C::heading("Application"),
                C::toggle("d.tray", "Show tray icon", cfg.show_tray_icon),
                C::toggle("d.auto", "Start on login", cfg.autostart_enabled),
                C::toggle("d.ui", "Open UI on launch", cfg.open_ui_on_launch),
                C::toggle("d.net", "Allow network access", cfg.allow_network_access),
                C::note("Network access lets other computers on your network control the mixer."),
            ];
            if let Some(t) = cfg.tts_enabled {
                out.push(C::toggle("d.tts", "Text to speech", t));
            }
            out.push(C::choice("d.log", "Log level", LOG_LEVELS.iter().map(|s| s.to_string()).collect(), cfg.log_level_index()));
            out.push(C::choice("d.fw", "Firmware channel", vec!["Live".into(), "Beta".into()], cfg.firmware_index()));
            out.push(C::heading("Danger zone"));
            out.push(C::button("d.stop", "Daemon", if confirm == Some("d.stop") { "Confirm stop?" } else { "Stop daemon" }, true));
            out
        }
        _ => {
            let mut out = vec![C::heading("Daemon"), C::info("Version", &cfg.daemon_version), C::info("Platform", &cfg.platform)];
            if let Some(m) = m {
                let h = &m.hardware;
                out.push(C::heading("Device"));
                out.push(C::info("Model", &format!("{:?}", h.device_type)));
                out.push(C::info("Serial", &h.serial_number));
                out.push(C::info("Manufactured", &h.manufactured_date));
                out.push(C::info("Firmware", &h.versions.firmware.to_string()));
                out.push(C::info("USB product", &h.usb_device.product_name));
            }
            out
        }
    }
}

trait CfgIdx {
    fn log_level_index(&self) -> i32;
    fn firmware_index(&self) -> i32;
}

impl CfgIdx for goxlr_ipc::DaemonConfig {
    fn log_level_index(&self) -> i32 {
        use goxlr_ipc::LogLevel::*;
        match self.log_level {
            Off => 0,
            Error => 1,
            Warn => 2,
            Info => 3,
            Debug => 4,
            Trace => 5,
        }
    }
    fn firmware_index(&self) -> i32 {
        match self.firmware_source {
            goxlr_ipc::FirmwareSource::Live => 0,
            goxlr_ipc::FirmwareSource::Beta => 1,
        }
    }
}
