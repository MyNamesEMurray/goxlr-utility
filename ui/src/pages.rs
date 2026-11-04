use crate::import::{self, ImportState};
use crate::model::{C, hex_to_rgb, idx, opts, pretty};
use crate::table;
use goxlr_ipc::{DaemonStatus, MixerStatus, PathTypes, UpdateState};
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
    pub imp: &'a ImportState,
}

pub fn tabs(page: u8) -> Vec<&'static str> {
    match page {
        2 => vec!["Input", "Gate", "Compressor", "Equaliser"],
        3 => vec![
            "Presets",
            "Reverb",
            "Echo",
            "Pitch",
            "Gender",
            "Megaphone",
            "Robot",
            "Hard tune",
        ],
        4 => vec!["Bank A", "Bank B", "Bank C", "Files"],
        5 => vec!["Animation", "Faders", "Buttons", "Other"],
        6 => vec!["Profiles", "Mic profiles", "Folders", "Import"],
        7 => vec!["Mixer", "Daemon", "Firmware", "Device"],
        _ => vec![],
    }
}

fn names<T: IntoEnumIterator + std::fmt::Display>() -> Vec<String> {
    opts::<T>()
}

pub fn mixer(c: &Ctx) -> Vec<C> {
    let m = c.m;
    let mut out = vec![
        C::heading("Microphone"),
        C::toggle(
            "mix.cough",
            "Cough button muted",
            m.cough_button.state != MuteState::Unmuted,
        ),
        C::slider(
            "mix.bleep",
            "Bleep volume",
            -36.0,
            0.0,
            1.0,
            m.levels.bleep as f32,
            " dB",
        ),
        C::heading("Faders"),
    ];
    let inputs: Vec<String> = ChannelName::iter()
        .take(8)
        .map(|c| pretty(&c.to_string()))
        .collect();
    for f in FaderName::iter() {
        let fs = &m.fader_status[f];
        let ci = ChannelName::iter()
            .take(8)
            .position(|x| x == fs.channel)
            .map_or(-1, |i| i as i32);
        out.push(C::choice(
            &format!("mix.fch.{f}"),
            &format!("Fader {f} channel"),
            inputs.clone(),
            ci,
        ));
        out.push(C::choice(
            &format!("mix.ffn.{f}"),
            &format!("Fader {f} mute"),
            names::<MuteFunction>(),
            idx(&fs.mute_type),
        ));
    }
    out.push(C::toggle("mix.lock", "Lock faders", m.settings.lock_faders));
    if m.hardware.device_type == DeviceType::Full {
        out.push(C::heading("Scribbles"));
        let mut icon_opts = vec!["None".to_string()];
        icon_opts.extend(c.st.files.icons.iter().cloned());
        for f in FaderName::iter() {
            let s = m.fader_status[f].scribble.clone().unwrap_or_default();
            let ii = s
                .file_name
                .as_ref()
                .and_then(|n| c.st.files.icons.iter().position(|i| i == n))
                .map_or(0, |i| i as i32 + 1);
            out.push(C::choice(
                &format!("scr.icon.{f}"),
                &format!("{f} icon"),
                icon_opts.clone(),
                ii,
            ));
            out.push(C::field(
                &format!("scr.top.{f}"),
                &format!("{f} top text"),
                s.left_text.as_deref().unwrap_or(""),
            ));
            out.push(C::field(
                &format!("scr.bot.{f}"),
                &format!("{f} bottom text"),
                s.bottom_text.as_deref().unwrap_or(""),
            ));
            out.push(C::toggle(
                &format!("scr.inv.{f}"),
                &format!("{f} inverted"),
                s.inverted,
            ));
        }
    }
    out
}

pub fn routing(c: &Ctx) -> Vec<C> {
    let m = c.m;
    let mut out = vec![
        C::heading("Monitor"),
        C::choice(
            "mon.mix",
            "Monitor mix",
            names::<OutputDevice>(),
            idx(&m.levels.output_monitor),
        ),
        C::toggle(
            "mon.fx",
            "Monitor with effects",
            m.settings.enable_monitor_with_fx,
        ),
    ];
    if m.levels.submix_supported {
        out.push(C::heading("Sub mix"));
        out.push(C::toggle(
            "sub.on",
            "Enable sub mix",
            m.levels.submix.is_some(),
        ));
        if let Some(sub) = &m.levels.submix {
            for (i, ch) in SubMixChannelName::iter().enumerate() {
                let s = &sub.inputs[ch];
                out.push(C::slider_link(
                    &format!("sub.vol.{i}"),
                    &pretty(&ch.to_string()),
                    0.0,
                    255.0,
                    s.volume as f32,
                    &format!("sub.link.{i}"),
                    s.linked,
                ));
            }
            out.push(C::heading("Output mix"));
            for (i, o) in OutputDevice::iter().enumerate() {
                out.push(C::seg(
                    &format!("sub.out.{i}"),
                    &pretty(&o.to_string()),
                    vec!["A".into(), "B".into()],
                    idx(&sub.outputs[o]),
                ));
            }
        }
    }
    out
}

pub fn matrix(m: &MixerStatus) -> (Vec<String>, Vec<String>, Vec<Vec<i32>>) {
    let inputs: Vec<String> = InputDevice::iter()
        .map(|i| pretty(&i.to_string()))
        .collect();
    let outputs: Vec<String> = OutputDevice::iter()
        .map(|o| pretty(&o.to_string()))
        .collect();
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

pub fn mic(c: &Ctx, tab: u8) -> Vec<C> {
    if tab != 3 {
        return table::build(table::MIC, tab, c.m);
    }
    let m = c.m;
    let mut out = vec![C::heading("Gain")];
    let mut freqs = vec![C::heading("Centre frequency (Hz)")];
    if m.hardware.device_type == DeviceType::Mini {
        for (i, b) in MiniEqFrequencies::iter().enumerate() {
            let g = m
                .mic_status
                .equaliser_mini
                .gain
                .get(&b)
                .copied()
                .unwrap_or(0);
            out.push(C::slider(
                &format!("eq.gain.{i}"),
                &pretty(&b.to_string()),
                -9.0,
                9.0,
                1.0,
                g as f32,
                " dB",
            ));
            let f = m
                .mic_status
                .equaliser_mini
                .frequency
                .get(&b)
                .copied()
                .unwrap_or(0.0);
            freqs.push(C::field(
                &format!("eq.freq.{i}"),
                &pretty(&b.to_string()),
                &format!("{}", f.round()),
            ));
        }
    } else {
        for (i, b) in EqFrequencies::iter().enumerate() {
            let g = m.mic_status.equaliser.gain.get(&b).copied().unwrap_or(0);
            out.push(C::slider(
                &format!("eq.gain.{i}"),
                &pretty(&b.to_string()),
                -9.0,
                9.0,
                1.0,
                g as f32,
                " dB",
            ));
            let f = m
                .mic_status
                .equaliser
                .frequency
                .get(&b)
                .copied()
                .unwrap_or(0.0);
            freqs.push(C::field(
                &format!("eq.freq.{i}"),
                &pretty(&b.to_string()),
                &format!("{}", f.round()),
            ));
        }
    }
    out.extend(freqs);
    out
}

pub fn effects(c: &Ctx, tab: u8) -> Vec<C> {
    let Some(e) = &c.m.effects else {
        return vec![C::note("Effects are not available on this device.")];
    };
    if tab != 0 {
        return table::build(table::FX, tab, c.m);
    }
    let active = e
        .preset_names
        .get(&e.active_preset)
        .cloned()
        .unwrap_or_default();
    vec![
        C::heading("Presets"),
        C::toggle("fx.on", "Effects enabled", e.is_enabled),
        C::seg(
            "fx.preset",
            "Active preset",
            (1..=6).map(|i| i.to_string()).collect(),
            idx(&e.active_preset),
        ),
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
            C::seg(
                "samp.active",
                "Active bank",
                vec!["A".into(), "B".into(), "C".into()],
                idx(&s.active_bank),
            ),
            C::toggle(
                "samp.reset",
                "Reset on clear",
                c.m.settings.reset_sampler_on_clear,
            ),
            C::slider(
                "samp.fade",
                "Fade time",
                0.0,
                20000.0,
                100.0,
                c.m.settings.fade_duration as f32,
                " ms",
            ),
            C::slider(
                "samp.buf",
                "Record buffer",
                0.0,
                30000.0,
                500.0,
                s.record_buffer as f32,
                " ms",
            ),
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
            out.push(C::slider(
                &format!("samp.gain.{i}"),
                &f.name,
                0.0,
                100.0,
                1.0,
                f.gain_pct as f32,
                "%",
            ));
        }
        return out;
    }
    let bank = SampleBank::iter()
        .nth(tab as usize)
        .unwrap_or(SampleBank::A);
    let mut out = vec![];
    out.push(C::seg(
        "samp.active",
        "Bank used by the device",
        vec!["A".into(), "B".into(), "C".into()],
        idx(&s.active_bank),
    ));
    let Some(btns) = s.banks.get(&bank) else {
        return out;
    };
    let sample_names: Vec<String> = c.st.files.samples.keys().cloned().collect();
    for b in SampleButtons::iter() {
        let Some(btn) = btns.get(&b) else { continue };
        out.push(C::heading(&format!(
            "{}{}",
            pretty(&b.to_string()),
            if btn.is_playing { "  (playing)" } else { "" }
        )));
        out.push(C::choice(
            &format!("samp.mode.{bank}.{b}"),
            "Mode",
            names::<SamplePlaybackMode>(),
            idx(&btn.function),
        ));
        out.push(C::seg(
            &format!("samp.order.{bank}.{b}"),
            "Order",
            names::<SamplePlayOrder>(),
            idx(&btn.order),
        ));
        for (i, smp) in btn.samples.iter().enumerate() {
            let k = format!("{bank}.{b}.{i}");
            let confirm_key = format!("samp.s.{k}|1");
            out.push(C::item(
                &format!("samp.s.{k}"),
                &smp.name,
                false,
                true,
                [
                    "Play",
                    if c.confirm == Some(&confirm_key) {
                        "Confirm?"
                    } else {
                        "Remove"
                    },
                ],
            ));
            out.push(C::slider(
                &format!("samp.start.{k}"),
                "Start",
                0.0,
                100.0,
                0.1,
                smp.start_pct,
                "%",
            ));
            out.push(C::slider(
                &format!("samp.end.{k}"),
                "End",
                0.0,
                100.0,
                0.1,
                smp.stop_pct,
                "%",
            ));
        }
        out.push(C::choice(
            &format!("samp.add.{bank}.{b}"),
            "Add sample",
            sample_names.clone(),
            -1,
        ));
        out.push(C::button(
            &format!("samp.halt.{bank}.{b}"),
            "",
            "Stop playback",
            false,
        ));
    }
    out
}

fn rgb(s: &str) -> (u8, u8, u8) {
    hex_to_rgb(s)
}

pub fn lighting(c: &Ctx, tab: u8) -> Vec<C> {
    let l = &c.m.lighting;
    let mut out = vec![];
    match tab {
        0 => {
            if !l.animation.supported {
                return vec![C::note("Animations are not supported on this device.")];
            }
            out.push(C::choice(
                "light.anim",
                "Mode",
                names::<AnimationMode>(),
                idx(&l.animation.mode),
            ));
            out.push(C::slider(
                "light.mod1",
                "Modifier 1",
                0.0,
                100.0,
                1.0,
                l.animation.mod1 as f32,
                "",
            ));
            out.push(C::slider(
                "light.mod2",
                "Modifier 2",
                0.0,
                100.0,
                1.0,
                l.animation.mod2 as f32,
                "",
            ));
            out.push(C::seg(
                "light.water",
                "Waterfall",
                names::<WaterfallDirection>(),
                idx(&l.animation.waterfall_direction),
            ));
        }
        1 => {
            let styles = names::<FaderDisplayStyle>();
            out.push(C::heading("All faders"));
            let first = FaderName::iter().find_map(|f| l.faders.get(&f));
            if let Some(f) = first {
                out.push(C::colours(
                    "light.allf",
                    "Colours",
                    vec![rgb(&f.colours.colour_one), rgb(&f.colours.colour_two)],
                ));
            }
            out.push(C::choice("light.allstyle", "Style", styles.clone(), -1));
            for f in FaderName::iter() {
                let Some(fl) = l.faders.get(&f) else { continue };
                out.push(C::heading(&format!("Fader {f}")));
                out.push(C::colours(
                    &format!("light.fader.{f}"),
                    "Colours",
                    vec![rgb(&fl.colours.colour_one), rgb(&fl.colours.colour_two)],
                ));
                out.push(C::choice(
                    &format!("light.fstyle.{f}"),
                    "Style",
                    styles.clone(),
                    idx(&fl.style),
                ));
            }
        }
        2 => {
            for (i, b) in Button::iter().enumerate() {
                let Some(bl) = l.buttons.get(&b) else {
                    continue;
                };
                out.push(C::heading(&pretty(&b.to_string())));
                out.push(C::colours(
                    &format!("light.btn.{i}"),
                    "On / off colour",
                    vec![rgb(&bl.colours.colour_one), rgb(&bl.colours.colour_two)],
                ));
                out.push(C::choice(
                    &format!("light.boff.{i}"),
                    "When off",
                    names::<ButtonColourOffStyle>(),
                    idx(&bl.off_style),
                ));
            }
        }
        _ => {
            out.push(C::heading("General"));
            for (i, t) in SimpleColourTargets::iter().enumerate() {
                if let Some(s) = l.simple.get(&t) {
                    out.push(C::colours(
                        &format!("light.simple.{i}"),
                        &pretty(&t.to_string()),
                        vec![rgb(&s.colour_one)],
                    ));
                }
            }
            if !l.encoders.is_empty() {
                out.push(C::heading("Encoders"));
            }
            for (i, t) in EncoderColourTargets::iter().enumerate() {
                if let Some(e) = l.encoders.get(&t) {
                    out.push(C::colours(
                        &format!("light.enc.{i}"),
                        &pretty(&t.to_string()),
                        vec![rgb(&e.colour_one), rgb(&e.colour_two), rgb(&e.colour_three)],
                    ));
                }
            }
            if !l.sampler.is_empty() {
                out.push(C::heading("Sampler banks"));
            }
            for (i, t) in SamplerColourTargets::iter().enumerate() {
                if let Some(s) = l.sampler.get(&t) {
                    out.push(C::colours(
                        &format!("light.samp.{i}"),
                        &pretty(&t.to_string()),
                        vec![
                            rgb(&s.colours.colour_one),
                            rgb(&s.colours.colour_two),
                            rgb(&s.colours.colour_three),
                        ],
                    ));
                    out.push(C::choice(
                        &format!("light.soff.{i}"),
                        "When off",
                        names::<ButtonColourOffStyle>(),
                        idx(&s.off_style),
                    ));
                }
            }
        }
    }
    out
}

pub fn profiles(c: &Ctx, tab: u8) -> Vec<C> {
    let (prefix, files, current, kind): (&str, &Vec<String>, &str, &str) = match tab {
        0 => ("prof", &c.st.files.profiles, &c.m.profile_name, "profile"),
        1 => (
            "mprof",
            &c.st.files.mic_profiles,
            &c.m.mic_profile_name,
            "mic profile",
        ),
        3 => return import_rows(c),
        _ => {
            let mut out = vec![];
            for (i, (_, name)) in PATHS.iter().enumerate() {
                let k = format!("path.{i}");
                let ck = format!("{k}|1");
                out.push(C::item(
                    &k,
                    name,
                    false,
                    true,
                    [
                        "Open",
                        if c.confirm == Some(&ck) {
                            "Confirm restore?"
                        } else {
                            "Restore defaults"
                        },
                    ],
                ));
            }
            return out;
        }
    };
    let mut out = vec![C::heading(if tab == 0 {
        "Profiles"
    } else {
        "Mic profiles"
    })];
    for (i, n) in files.iter().enumerate() {
        let k = format!("{prefix}.{i}");
        let ck = format!("{k}|1");
        out.push(C::item(
            &k,
            n,
            n == current,
            n != current,
            [
                "Load",
                if c.confirm == Some(&ck) {
                    "Confirm delete?"
                } else {
                    "Delete"
                },
            ],
        ));
    }
    out.push(C::heading("Manage"));
    out.push(C::button(
        &format!("{prefix}.save"),
        &format!("Save changes to this {kind}"),
        "Save",
        false,
    ));
    out.push(C::field(&format!("{prefix}.saveas"), "Save as a copy", ""));
    out.push(C::field(&format!("{prefix}.new"), "Create a new one", ""));
    out
}

pub fn settings(
    st: &DaemonStatus,
    m: Option<&MixerStatus>,
    confirm: Option<&str>,
    tab: u8,
    fw_file: &str,
) -> Vec<C> {
    let cfg = &st.config;
    match tab {
        0 => {
            let Some(m) = m else {
                return vec![C::note("No GoXLR detected.")];
            };
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
                out.push(C::seg(
                    &format!("set.disp.{i}"),
                    &pretty(&comp.to_string()),
                    names::<DisplayMode>(),
                    idx(&cur),
                ));
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
            out.push(C::choice(
                "d.log",
                "Log level",
                LOG_LEVELS.iter().map(|s| s.to_string()).collect(),
                cfg.log_level_index(),
            ));
            if cfg.platform == "macos" {
                out.push(C::toggle(
                    "d.macagg",
                    "Handle macOS aggregate devices",
                    cfg.handle_macos_aggregates,
                ));
            }
            out.push(C::field(
                "d.locale",
                "Language override",
                cfg.locale.user_locale.as_deref().unwrap_or(""),
            ));
            out.push(C::info("System language", &cfg.locale.system_locale));
            out.push(C::heading("Danger zone"));
            out.push(C::button(
                "d.stop",
                "Daemon",
                if confirm == Some("d.stop") {
                    "Confirm stop?"
                } else {
                    "Stop daemon"
                },
                true,
            ));
            out
        }
        2 => firmware_rows(st, m, confirm, fw_file),
        _ => {
            let mut out = vec![
                C::heading("Daemon"),
                C::info("Version", &cfg.daemon_version),
                C::info("Platform", &cfg.platform),
            ];
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

fn import_rows(c: &Ctx) -> Vec<C> {
    let s = c.imp;
    let mut out = vec![
        C::heading("Import from the official GoXLR app"),
        C::note(
            "Copies profiles and mic profiles into this utility. Close the official app first.",
        ),
        C::field("imp.path", "Folder to import from", &s.path),
        C::button("imp.scan", "", "Scan for profiles", false),
        C::toggle("imp.over", "Replace existing files", s.overwrite),
    ];
    if s.scanned && s.files.is_empty() {
        out.push(C::info("Result", "No profiles found in that folder."));
        return out;
    }
    if !s.files.is_empty() {
        out.push(C::heading(&format!("Found {} files", s.files.len())));
        for (i, f) in s.files.iter().enumerate() {
            let exists = import::dest_exists(
                f,
                &c.st.paths.profile_directory,
                &c.st.paths.mic_profile_directory,
            );
            let kind = if f.mic { "mic profile" } else { "profile" };
            let note = if exists { ", already exists" } else { "" };
            out.push(C::toggle(
                &format!("imp.sel.{i}"),
                &format!("{} ({kind}{note})", f.name()),
                f.selected,
            ));
        }
        let n = s.files.iter().filter(|f| f.selected).count();
        out.push(C::button(
            "imp.go",
            "",
            &format!("Import {n} selected"),
            false,
        ));
    }
    if !s.msg.is_empty() {
        out.push(C::info("Result", &s.msg));
    }
    out
}

fn firmware_rows(
    st: &DaemonStatus,
    m: Option<&MixerStatus>,
    confirm: Option<&str>,
    fw_file: &str,
) -> Vec<C> {
    let Some(m) = m else {
        return vec![C::note("No GoXLR detected.")];
    };
    let current = m.hardware.versions.firmware.to_string();
    let latest = st
        .config
        .latest_firmware
        .as_ref()
        .and_then(|l| l[m.hardware.device_type].as_ref())
        .map_or_else(|| "unknown".to_string(), |d| d.version.to_string());
    let mut out = vec![
        C::heading("Firmware"),
        C::info("Installed", &current),
        C::info("Latest available", &latest),
        C::seg(
            "d.fw",
            "Update channel",
            vec!["Live".into(), "Beta".into()],
            st.config.firmware_index(),
        ),
    ];
    let busy_label = |state: &UpdateState| match state {
        UpdateState::Starting => "Starting",
        UpdateState::Manifest => "Checking for firmware",
        UpdateState::Download => "Downloading",
        UpdateState::ClearNVR => "Preparing the device",
        UpdateState::UploadFirmware => "Uploading",
        UpdateState::ValidateUpload => "Checking the upload",
        UpdateState::HardwareValidate => "Verifying on the device",
        UpdateState::HardwareWrite => "Writing to the device",
        _ => "",
    };
    match st.firmware.get(&m.hardware.serial_number) {
        Some(f) if matches!(f.state, UpdateState::Complete) => {
            out.push(C::info(
                "Status",
                "Update complete. The device is restarting.",
            ));
            out.push(C::button("fw.clear", "", "Dismiss", false));
        }
        Some(f) if matches!(f.state, UpdateState::Failed) => {
            out.push(C::info("Status", "Update failed"));
            out.push(C::info(
                "Error",
                f.error.as_deref().unwrap_or("unknown error"),
            ));
            out.push(C::button("fw.clear", "", "Dismiss", false));
        }
        Some(f) => {
            if let UpdateState::Pause(info) = &f.state {
                out.push(C::note(&format!("The firmware found ({}) is not newer than the one installed. Install it anyway?", info.version)));
                out.push(C::button("fw.continue", "", "Install anyway", true));
                out.push(C::button("fw.clear", "", "Cancel", false));
            } else {
                out.push(C::info(
                    "Status",
                    &format!("{} ({}%)", busy_label(&f.state), f.progress),
                ));
                out.push(C::note(
                    "Do not unplug the device or close the utility until this finishes.",
                ));
            }
        }
        None => {
            out.push(C::heading("Install"));
            out.push(C::note(
                "Do not unplug the device or close the utility during an update.",
            ));
            out.push(C::button(
                "fw.update",
                "Latest firmware",
                if confirm == Some("fw.update") {
                    "Confirm update?"
                } else {
                    "Download and install"
                },
                confirm == Some("fw.update"),
            ));
            out.push(C::field("fw.file", "Firmware file (.bin)", fw_file));
            out.push(C::button(
                "fw.install_file",
                "",
                if confirm == Some("fw.install_file") {
                    "Confirm install?"
                } else {
                    "Install this file"
                },
                confirm == Some("fw.install_file"),
            ));
        }
    }
    out
}
