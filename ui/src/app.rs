use crate::model::{C, nth, pretty, sync};
use crate::pages::{self, Ctx, LOG_LEVELS, PATHS};
use crate::table;
use crate::{AppWindow, Ctl, Strip};
use goxlr_ipc::{DaemonCommand, DaemonStatus, FirmwareSource, GoXLRCommand as G, LogLevel, MixerStatus};
use goxlr_types::*;
use slint::{ComponentHandle, Model, ModelRc, SharedString, Timer, TimerMode, VecModel, Weak};
use std::collections::HashMap;
use std::rc::Rc;
use std::time::{Duration, Instant};
use strum::IntoEnumIterator;
use tokio::sync::mpsc::UnboundedSender;

pub enum Msg {
    Cmd(String, G),
    Daemon(DaemonCommand),
    Mic(Option<String>),
}

const HOLD: Duration = Duration::from_millis(450);

pub struct App {
    ui: Weak<AppWindow>,
    tx: UnboundedSender<Msg>,
    status: Option<DaemonStatus>,
    serial: Option<String>,
    confirm: Option<String>,
    touched: HashMap<String, Instant>,
    models: Vec<Rc<VecModel<Ctl>>>,
    prev: Vec<Vec<C>>,
    strips: Rc<VecModel<Strip>>,
    inputs: Rc<VecModel<SharedString>>,
    outputs: Rc<VecModel<SharedString>>,
    matrix: Rc<VecModel<ModelRc<i32>>>,
    prev_matrix: Vec<Vec<i32>>,
    last_mic: Option<String>,
    apply_timer: Timer,
}

fn fader_by_name(s: &str) -> Option<FaderName> {
    FaderName::iter().find(|f| f.to_string() == s)
}

fn bank_by_name(s: &str) -> Option<SampleBank> {
    SampleBank::iter().find(|f| f.to_string() == s)
}

fn button_by_name(s: &str) -> Option<SampleButtons> {
    SampleButtons::iter().find(|f| f.to_string() == s)
}

impl App {
    pub fn new(ui: &AppWindow, tx: UnboundedSender<Msg>) -> App {
        let models: Vec<Rc<VecModel<Ctl>>> = (0..8).map(|_| Rc::new(VecModel::default())).collect();
        let app = App {
            ui: ui.as_weak(),
            tx,
            status: None,
            serial: None,
            confirm: None,
            touched: HashMap::new(),
            prev: vec![vec![]; 8],
            strips: Rc::new(VecModel::default()),
            inputs: Rc::new(VecModel::default()),
            outputs: Rc::new(VecModel::default()),
            matrix: Rc::new(VecModel::default()),
            prev_matrix: vec![],
            last_mic: None,
            apply_timer: Timer::default(),
            models,
        };
        ui.set_mixer_ctls(app.models[0].clone().into());
        ui.set_routing_ctls(app.models[1].clone().into());
        ui.set_mic_ctls(app.models[2].clone().into());
        ui.set_fx_ctls(app.models[3].clone().into());
        ui.set_sampler_ctls(app.models[4].clone().into());
        ui.set_light_ctls(app.models[5].clone().into());
        ui.set_profile_ctls(app.models[6].clone().into());
        ui.set_settings_ctls(app.models[7].clone().into());
        ui.set_strips(app.strips.clone().into());
        ui.set_inputs(app.inputs.clone().into());
        ui.set_outputs(app.outputs.clone().into());
        ui.set_matrix(app.matrix.clone().into());
        app
    }

    pub fn set_mic_level(&self, level: f32) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_mic_level(level);
        }
    }

    pub fn set_error(&self, msg: &str) {
        if let Some(ui) = self.ui.upgrade() {
            ui.set_error_text(msg.into());
        }
    }

    pub fn on_status(&mut self, st: Option<DaemonStatus>) {
        if let Some(st) = &st {
            let mut serials: Vec<&String> = st.mixers.keys().collect();
            serials.sort();
            if self.serial.as_ref().is_none_or(|s| !st.mixers.contains_key(s)) {
                self.serial = serials.first().map(|s| s.to_string());
            }
        }
        self.status = st;
        self.refresh();
    }

    fn tab_of(ui: &AppWindow, page: u8) -> u8 {
        (match page {
            2 => ui.get_mic_tab(),
            3 => ui.get_fx_tab(),
            4 => ui.get_sampler_tab(),
            5 => ui.get_light_tab(),
            6 => ui.get_profile_tab(),
            7 => ui.get_settings_tab(),
            _ => 0,
        }) as u8
    }

    fn mixer(&self) -> Option<&MixerStatus> {
        self.status.as_ref().and_then(|s| self.serial.as_ref().and_then(|k| s.mixers.get(k)))
    }

    pub fn refresh(&mut self) {
        let Some(ui) = self.ui.upgrade() else { return };
        ui.set_online(self.status.is_some());
        let page = ui.get_page() as u8;
        let tab = Self::tab_of(&ui, page);

        let want_mic = if page == 0 { self.serial.clone().filter(|_| self.status.is_some()) } else { None };
        if want_mic != self.last_mic {
            let _ = self.tx.send(Msg::Mic(want_mic.clone()));
            self.last_mic = want_mic;
        }

        let Some(st) = &self.status else {
            ui.set_device_text("".into());
            return;
        };
        let m = self.mixer();
        ui.set_device_text(m.map_or(String::new(), |m| format!("{}  /  {}", m.profile_name, m.mic_profile_name)).into());

        let mut strips: Option<Vec<Strip>> = None;
        let mut matrix: Option<(Vec<String>, Vec<String>, Vec<Vec<i32>>)> = None;
        let rows: Vec<C> = match m {
            None if page == 7 => pages::settings(st, None, self.confirm.as_deref(), tab),
            None => vec![C::note("No GoXLR detected. Plug in a device and it will appear here.")],
            Some(m) => {
                let c = Ctx { st, m, confirm: self.confirm.as_deref() };
                match page {
                    0 => {
                        strips = Some(
                            ChannelName::iter()
                                .map(|ch| {
                                    let f = FaderName::iter().find(|f| m.fader_status[*f].channel == ch);
                                    Strip {
                                        key: ch.to_string().into(),
                                        name: pretty(&ch.to_string()).into(),
                                        fader: f.map_or(String::new(), |f| f.to_string()).into(),
                                        volume: m.levels.volumes[ch] as f32,
                                        muted: f.is_some_and(|f| m.fader_status[f].mute_state != MuteState::Unmuted),
                                    }
                                })
                                .collect(),
                        );
                        pages::mixer(&c)
                    }
                    1 => {
                        matrix = Some(pages::matrix(m));
                        pages::routing(&c)
                    }
                    2 => pages::mic(&c, tab),
                    3 => pages::effects(&c, tab),
                    4 => pages::sampler(&c, tab),
                    5 => pages::lighting(&c, tab),
                    6 => pages::profiles(&c, tab),
                    _ => pages::settings(st, Some(m), self.confirm.as_deref(), tab),
                }
            }
        };

        let touched = &self.touched;
        let skip = |k: &str| touched.get(k).is_some_and(|t| t.elapsed() < HOLD);
        let p = page as usize;
        sync(&self.models[p], &mut self.prev[p], rows, &skip);

        if let Some(new) = strips {
            if self.strips.row_count() != new.len() {
                self.strips.set_vec(new);
            } else {
                for (i, s) in new.into_iter().enumerate() {
                    if self.strips.row_data(i).as_ref() != Some(&s) && !skip(&format!("strip.{}", s.key)) {
                        self.strips.set_row_data(i, s);
                    }
                }
            }
        }
        if let Some((inputs, outputs, rows)) = matrix {
            if self.inputs.row_count() == 0 {
                self.inputs.set_vec(inputs.into_iter().map(SharedString::from).collect::<Vec<_>>());
                self.outputs.set_vec(outputs.into_iter().map(SharedString::from).collect::<Vec<_>>());
            }
            if rows != self.prev_matrix && !skip("route") {
                self.matrix.set_vec(rows.iter().map(|r| ModelRc::new(VecModel::from(r.clone()))).collect::<Vec<_>>());
                self.prev_matrix = rows;
            }
        }
    }

    fn cmd(&self, c: G) {
        if let Some(s) = &self.serial {
            let _ = self.tx.send(Msg::Cmd(s.clone(), c));
        }
    }

    fn daemon(&self, c: DaemonCommand) {
        let _ = self.tx.send(Msg::Daemon(c));
    }

    fn touch(&mut self, key: &str) {
        self.touched.insert(key.to_string(), Instant::now());
    }

    fn optimistic(&mut self, key: &str, value: f32) {
        let Some(ui) = self.ui.upgrade() else { return };
        let p = ui.get_page() as usize;
        let (base, ci) = match key.split_once(':') {
            Some((b, c)) => (b, c.parse::<usize>().ok()),
            None => (key, None),
        };
        if let Some(i) = self.prev[p].iter().position(|c| c.kind == 11 && c.text == key) {
            self.prev[p][i].checked = value > 0.5;
            self.models[p].set_row_data(i, self.prev[p][i].to_ctl());
            return;
        }
        let Some(i) = self.prev[p].iter().position(|c| c.key == base) else { return };
        let c = &mut self.prev[p][i];
        match (c.kind, ci) {
            (0, _) => c.value = value,
            (1, _) => c.checked = value > 0.5,
            (2 | 3, _) => c.index = value as i32,
            (5, Some(ci)) if ci < c.colours.len() => {
                let n = value as u32;
                c.colours[ci] = (((n >> 16) & 255) as u8, ((n >> 8) & 255) as u8, (n & 255) as u8);
            }
            _ => return,
        }
        self.models[p].set_row_data(i, c.to_ctl());
    }

    fn colours_of(&self, base: &str) -> Vec<String> {
        self.prev
            .iter()
            .flatten()
            .find(|c| c.key == base && c.kind == 5)
            .map(|c| c.colours.iter().map(|c| format!("{:02X}{:02X}{:02X}", c.0, c.1, c.2)).collect())
            .unwrap_or_default()
    }

    pub fn strip_volume(&mut self, key: &str, v: f32) {
        if let Some(ch) = ChannelName::iter().find(|c| c.to_string() == key) {
            self.touch(&format!("strip.{key}"));
            if let Some(i) = (0..self.strips.row_count()).find(|i| self.strips.row_data(*i).is_some_and(|s| s.key == key)) {
                let mut s = self.strips.row_data(i).unwrap();
                s.volume = v;
                self.strips.set_row_data(i, s);
            }
            self.cmd(G::SetVolume(ch, v as u8));
        }
    }

    pub fn strip_mute(&mut self, key: &str) {
        let Some(m) = self.mixer() else { return };
        let Some(f) = FaderName::iter().find(|f| m.fader_status[*f].channel.to_string() == key) else { return };
        let next = if m.fader_status[f].mute_state == MuteState::Unmuted { MuteState::MutedToX } else { MuteState::Unmuted };
        self.cmd(G::SetFaderMuteState(f, next));
    }

    pub fn route(&mut self, i: usize, o: usize, on: bool) {
        let (Some(inp), Some(out)) = (nth::<InputDevice>(i), nth::<OutputDevice>(o)) else { return };
        self.touch("route");
        if let Some(cell) = self.prev_matrix.get_mut(i).and_then(|r| r.get_mut(o)) {
            *cell = on as i32;
            self.matrix.set_vec(self.prev_matrix.iter().map(|r| ModelRc::new(VecModel::from(r.clone()))).collect::<Vec<_>>());
        }
        self.cmd(G::SetRouter(inp, out, on));
    }

    pub fn event(&mut self, key: &str, value: f32, text: &str) {
        let confirmable = key.ends_with("|1") || key == "d.stop";
        if confirmable {
            if self.confirm.as_deref() != Some(key) {
                self.confirm = Some(key.to_string());
                self.refresh();
                return;
            }
            self.confirm = None;
        } else if self.confirm.take().is_some() {
            self.refresh();
        }
        self.touch(key);
        self.optimistic(key, value);

        if let Some(r) = table::find(key) {
            if let Some(m) = self.mixer() {
                if let Some(c) = table::command(r, m, value) {
                    self.cmd(c);
                }
            }
            return;
        }
        let parts: Vec<&str> = key.split(['.', ':', '|']).collect();
        let v = value;
        let b = value > 0.5;
        let full = self.mixer().is_some_and(|m| m.hardware.device_type == DeviceType::Full);
        let cmd: Option<G> = match parts.as_slice() {
            ["mix", "cough"] => Some(G::SetCoughMuteState(if b { MuteState::MutedToX } else { MuteState::Unmuted })),
            ["mix", "bleep"] => Some(G::SetSwearButtonVolume(v as i8)),
            ["mix", "lock"] => Some(G::SetLockFaders(b)),
            ["mix", "fch", f] => fader_by_name(f).zip(ChannelName::iter().nth(v as usize)).map(|(f, c)| G::SetFader(f, c)),
            ["mix", "ffn", f] => fader_by_name(f).zip(MuteFunction::iter().nth(v as usize)).map(|(f, c)| G::SetFaderMuteFunction(f, c)),
            ["scr", "icon", f] => fader_by_name(f).map(|f| {
                let icon = (v as usize).checked_sub(1).and_then(|i| self.status.as_ref().and_then(|s| s.files.icons.get(i).cloned()));
                G::SetScribbleIcon(f, icon)
            }),
            ["scr", "top", f] => fader_by_name(f).map(|f| G::SetScribbleNumber(f, text.to_string())),
            ["scr", "bot", f] => fader_by_name(f).map(|f| G::SetScribbleText(f, text.to_string())),
            ["scr", "inv", f] => fader_by_name(f).map(|f| G::SetScribbleInvert(f, b)),
            ["mon", "mix"] => nth::<OutputDevice>(v as usize).map(G::SetMonitorMix),
            ["mon", "fx"] => Some(G::SetMonitorWithFx(b)),
            ["sub", "on"] => Some(G::SetSubMixEnabled(b)),
            ["sub", "vol", i] => i.parse().ok().and_then(nth::<ChannelName>).map(|c| G::SetSubMixVolume(c, v as u8)),
            ["sub", "link", i] => i.parse().ok().and_then(nth::<ChannelName>).map(|c| G::SetSubMixLinked(c, b)),
            ["sub", "out", i] => i.parse().ok().and_then(nth::<OutputDevice>).zip(nth::<Mix>(v as usize)).map(|(o, m)| G::SetSubMixOutputMix(o, m)),
            ["eq", "gain", i] => i.parse::<usize>().ok().and_then(|i| {
                if full { nth::<EqFrequencies>(i).map(|f| G::SetEqGain(f, v as i8)) } else { nth::<MiniEqFrequencies>(i).map(|f| G::SetEqMiniGain(f, v as i8)) }
            }),
            ["eq", "freq", i] => text.trim().parse::<f32>().ok().and_then(|hz| {
                i.parse::<usize>().ok().and_then(|i| {
                    if full { nth::<EqFrequencies>(i).map(|f| G::SetEqFreq(f, hz)) } else { nth::<MiniEqFrequencies>(i).map(|f| G::SetEqMiniFreq(f, hz)) }
                })
            }),
            ["fx", "on"] => Some(G::SetFXEnabled(b)),
            ["fx", "preset"] => nth::<EffectBankPresets>(v as usize).map(G::SetActiveEffectPreset),
            ["fx", "save"] => Some(G::SaveActivePreset()),
            ["fx", "rename"] => (!text.trim().is_empty()).then(|| G::RenameActivePreset(text.trim().to_string())),
            ["fx", "load"] => self.status.as_ref().and_then(|s| s.files.presets.get(v as usize).cloned()).map(G::LoadEffectPreset),
            ["samp", "active"] => nth::<SampleBank>(v as usize).map(G::SetActiveSamplerBank),
            ["samp", "reset"] => Some(G::SetSamplerResetOnClear(b)),
            ["samp", "fade"] => Some(G::SetSamplerFadeDuration(v as u32)),
            ["samp", "buf"] => Some(G::SetSamplerPreBufferDuration(v as u16)),
            ["samp", "clearerr"] => Some(G::ClearSampleProcessError()),
            ["samp", "gain", i] => {
                if let Some(name) = i.parse::<usize>().ok().and_then(|i| self.status.as_ref().and_then(|s| s.files.samples.keys().nth(i).cloned())) {
                    self.daemon(DaemonCommand::SetSampleGainPct(name, v as u8));
                    let tx = self.tx.clone();
                    self.apply_timer.start(TimerMode::SingleShot, Duration::from_millis(600), move || {
                        let _ = tx.send(Msg::Daemon(DaemonCommand::ApplySampleChange));
                    });
                }
                None
            }
            ["samp", "mode", bk, bt] => bank_by_name(bk).zip(button_by_name(bt)).zip(nth::<SamplePlaybackMode>(v as usize)).map(|((a, b), m)| G::SetSamplerFunction(a, b, m)),
            ["samp", "order", bk, bt] => bank_by_name(bk).zip(button_by_name(bt)).zip(nth::<SamplePlayOrder>(v as usize)).map(|((a, b), m)| G::SetSamplerOrder(a, b, m)),
            ["samp", "s", bk, bt, i, act] => bank_by_name(bk).zip(button_by_name(bt)).zip(i.parse::<usize>().ok()).map(|((a, b), i)| {
                if *act == "0" { G::PlaySampleByIndex(a, b, i) } else { G::RemoveSampleByIndex(a, b, i) }
            }),
            ["samp", "start", bk, bt, i] => bank_by_name(bk).zip(button_by_name(bt)).zip(i.parse::<usize>().ok()).map(|((a, b), i)| G::SetSampleStartPercent(a, b, i, v)),
            ["samp", "end", bk, bt, i] => bank_by_name(bk).zip(button_by_name(bt)).zip(i.parse::<usize>().ok()).map(|((a, b), i)| G::SetSampleStopPercent(a, b, i, v)),
            ["samp", "add", bk, bt] => bank_by_name(bk)
                .zip(button_by_name(bt))
                .zip(self.status.as_ref().and_then(|s| s.files.samples.keys().nth(v as usize).cloned()))
                .map(|((a, b), n)| G::AddSample(a, b, n)),
            ["samp", "halt", bk, bt] => bank_by_name(bk).zip(button_by_name(bt)).map(|(a, b)| G::StopSamplePlayback(a, b)),
            ["set", "disp", i] => nth::<DisplayModeComponents>(i.parse().unwrap_or(99)).zip(nth::<DisplayMode>(v as usize)).map(|(c, m)| G::SetElementDisplayMode(c, m)),
            ["light", ..] => self.light_cmd(&parts, key, v, b),
            ["prof", ..] | ["mprof", ..] => {
                self.profile_cmd(&parts, text);
                None
            }
            ["path", i, act] => {
                if let Some((p, _)) = i.parse::<usize>().ok().and_then(|i| PATHS.get(i)) {
                    self.daemon(if *act == "0" { DaemonCommand::OpenPath(p.clone()) } else { DaemonCommand::RecoverDefaults(p.clone()) });
                }
                None
            }
            ["d", what] => {
                let c = match *what {
                    "tray" => Some(DaemonCommand::SetShowTrayIcon(b)),
                    "auto" => Some(DaemonCommand::SetAutoStartEnabled(b)),
                    "ui" => Some(DaemonCommand::SetUiLaunchOnLoad(b)),
                    "net" => Some(DaemonCommand::SetAllowNetworkAccess(b)),
                    "tts" => Some(DaemonCommand::SetTTSEnabled(b)),
                    "log" => LOG_LEVELS.get(v as usize).map(|l| {
                        DaemonCommand::SetLogLevel(match *l {
                            "Off" => LogLevel::Off,
                            "Error" => LogLevel::Error,
                            "Warn" => LogLevel::Warn,
                            "Info" => LogLevel::Info,
                            "Debug" => LogLevel::Debug,
                            _ => LogLevel::Trace,
                        })
                    }),
                    "fw" => Some(DaemonCommand::SetFirmwareSource(if v as usize == 1 { FirmwareSource::Beta } else { FirmwareSource::Live })),
                    "stop" => Some(DaemonCommand::StopDaemon),
                    _ => None,
                };
                if let Some(c) = c {
                    self.daemon(c);
                }
                None
            }
            _ => None,
        };
        if let Some(c) = cmd {
            self.cmd(c);
        }
    }

    fn profile_cmd(&mut self, parts: &[&str], text: &str) {
        let mic = parts[0] == "mprof";
        let Some(st) = &self.status else { return };
        let files = if mic { &st.files.mic_profiles } else { &st.files.profiles };
        let text = text.trim().to_string();
        let c = match parts {
            [_, "save"] => Some(if mic { G::SaveMicProfile() } else { G::SaveProfile() }),
            [_, "saveas"] if !text.is_empty() => Some(if mic { G::SaveMicProfileAs(text) } else { G::SaveProfileAs(text) }),
            [_, "new"] if !text.is_empty() => Some(if mic { G::NewMicProfile(text) } else { G::NewProfile(text) }),
            [_, i, act] => i.parse::<usize>().ok().and_then(|i| files.get(i).cloned()).map(|n| match (mic, *act) {
                (false, "0") => G::LoadProfile(n, true),
                (false, _) => G::DeleteProfile(n),
                (true, "0") => G::LoadMicProfile(n, true),
                (true, _) => G::DeleteMicProfile(n),
            }),
            _ => None,
        };
        if let Some(c) = c {
            self.cmd(c);
        }
    }

    fn light_cmd(&self, parts: &[&str], key: &str, v: f32, b: bool) -> Option<G> {
        let base = key.split(':').next().unwrap_or(key);
        let cols = self.colours_of(base);
        let c = |i: usize| cols.get(i).cloned().unwrap_or_else(|| "000000".into());
        let idx = |i: &&str| i.parse::<usize>().ok();
        match parts {
            ["light", "anim"] => nth::<AnimationMode>(v as usize).map(G::SetAnimationMode),
            ["light", "mod1"] => Some(G::SetAnimationMod1(v as u8)),
            ["light", "mod2"] => Some(G::SetAnimationMod2(v as u8)),
            ["light", "water"] => nth::<WaterfallDirection>(v as usize).map(G::SetAnimationWaterfall),
            ["light", "allf", _] => Some(G::SetAllFaderColours(c(0), c(1))),
            ["light", "allstyle"] => nth::<FaderDisplayStyle>(v as usize).map(G::SetAllFaderDisplayStyle),
            ["light", "fader", f, _] => fader_by_name(f).map(|f| G::SetFaderColours(f, c(0), c(1))),
            ["light", "fstyle", f] => fader_by_name(f).zip(nth::<FaderDisplayStyle>(v as usize)).map(|(f, s)| G::SetFaderDisplayStyle(f, s)),
            ["light", "btn", i, _] => idx(i).and_then(nth::<Button>).map(|t| G::SetButtonColours(t, c(0), Some(c(1)))),
            ["light", "boff", i] => idx(i).and_then(nth::<Button>).zip(nth::<ButtonColourOffStyle>(v as usize)).map(|(t, s)| G::SetButtonOffStyle(t, s)),
            ["light", "simple", i, _] => idx(i).and_then(nth::<SimpleColourTargets>).map(|t| {
                if t == SimpleColourTargets::Global { G::SetGlobalColour(c(0)) } else { G::SetSimpleColour(t, c(0)) }
            }),
            ["light", "enc", i, _] => idx(i).and_then(nth::<EncoderColourTargets>).map(|t| G::SetEncoderColour(t, c(0), c(1), c(2))),
            ["light", "samp", i, _] => idx(i).and_then(nth::<SamplerColourTargets>).map(|t| G::SetSampleColour(t, c(0), c(1), c(2))),
            ["light", "soff", i] => idx(i).and_then(nth::<SamplerColourTargets>).zip(nth::<ButtonColourOffStyle>(v as usize)).map(|(t, s)| G::SetSampleOffStyle(t, s)),
            _ => {
                let _ = b;
                None
            }
        }
    }
}
