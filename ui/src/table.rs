use crate::model::{C, idx, nth, opts};
use goxlr_ipc::{GoXLRCommand as G, MixerStatus};
use goxlr_types::*;
use std::sync::LazyLock;

pub enum K {
    Slider {
        min: f32,
        max: f32,
        unit: &'static str,
        get: fn(&MixerStatus) -> f32,
        set: fn(&MixerStatus, f32) -> G,
    },
    Toggle {
        get: fn(&MixerStatus) -> bool,
        set: fn(&MixerStatus, bool) -> G,
    },
    Choice {
        seg: bool,
        opts: fn() -> Vec<String>,
        get: fn(&MixerStatus) -> i32,
        set: fn(usize) -> Option<G>,
    },
}

pub struct Row {
    pub page: u8,
    pub tab: u8,
    pub sec: &'static str,
    pub key: &'static str,
    pub label: &'static str,
    pub k: K,
}

macro_rules! slider {
    ($p:expr, $t:expr, $sec:expr, $key:expr, $label:expr, $min:expr, $max:expr, $unit:expr, $get:expr, $set:expr) => {
        Row {
            page: $p,
            tab: $t,
            sec: $sec,
            key: $key,
            label: $label,
            k: K::Slider {
                min: $min as f32,
                max: $max as f32,
                unit: $unit,
                get: $get,
                set: $set,
            },
        }
    };
}

macro_rules! toggle {
    ($p:expr, $t:expr, $sec:expr, $key:expr, $label:expr, $get:expr, $set:expr) => {
        Row {
            page: $p,
            tab: $t,
            sec: $sec,
            key: $key,
            label: $label,
            k: K::Toggle {
                get: $get,
                set: $set,
            },
        }
    };
}

macro_rules! choice {
    ($p:expr, $t:expr, $sec:expr, $key:expr, $label:expr, $seg:expr, $ty:ty, $get:expr, $set:expr) => {
        Row {
            page: $p,
            tab: $t,
            sec: $sec,
            key: $key,
            label: $label,
            k: K::Choice {
                seg: $seg,
                opts: opts::<$ty>,
                get: |m: &MixerStatus| idx::<$ty>(&$get(m)),
                set: |i| nth::<$ty>(i).map($set),
            },
        }
    };
}

macro_rules! fx {
    ($($p:tt)+) => { |m: &MixerStatus| m.effects.as_ref().map_or(0.0, |e| e.current.$($p)+ as f32) };
}

macro_rules! fxb {
    ($($p:tt)+) => { |m: &MixerStatus| m.effects.as_ref().is_some_and(|e| e.current.$($p)+) };
}

pub const MIC: u8 = 2;
pub const FX: u8 = 3;
pub const SETTINGS: u8 = 7;

pub static ROWS: LazyLock<Vec<Row>> = LazyLock::new(|| {
    vec![
        choice!(
            MIC,
            0,
            "Input",
            "mic.type",
            "Type",
            true,
            MicrophoneType,
            |m: &MixerStatus| m.mic_status.mic_type,
            G::SetMicrophoneType
        ),
        slider!(
            MIC,
            0,
            "",
            "mic.gain",
            "Gain",
            0,
            72,
            " dB",
            |m| m.mic_status.mic_gains[m.mic_status.mic_type] as f32,
            |m, v| G::SetMicrophoneGain(m.mic_status.mic_type, v as u16)
        ),
        slider!(
            MIC,
            0,
            "",
            "mic.deess",
            "De-esser",
            0,
            100,
            "%",
            |m| m.levels.deess as f32,
            |_, v| G::SetDeeser(v as u8)
        ),
        toggle!(
            MIC,
            1,
            "Noise gate",
            "gate.on",
            "Enabled",
            |m| m.mic_status.noise_gate.enabled,
            |_, v| G::SetGateActive(v)
        ),
        slider!(
            MIC,
            1,
            "",
            "gate.thr",
            "Threshold",
            -59,
            0,
            " dB",
            |m| m.mic_status.noise_gate.threshold as f32,
            |_, v| G::SetGateThreshold(v as i8)
        ),
        slider!(
            MIC,
            1,
            "",
            "gate.att",
            "Attenuation",
            0,
            100,
            "%",
            |m| m.mic_status.noise_gate.attenuation as f32,
            |_, v| G::SetGateAttenuation(v as u8)
        ),
        choice!(
            MIC,
            1,
            "",
            "gate.atk",
            "Attack",
            false,
            GateTimes,
            |m: &MixerStatus| m.mic_status.noise_gate.attack,
            G::SetGateAttack
        ),
        choice!(
            MIC,
            1,
            "",
            "gate.rel",
            "Release",
            false,
            GateTimes,
            |m: &MixerStatus| m.mic_status.noise_gate.release,
            G::SetGateRelease
        ),
        slider!(
            MIC,
            2,
            "Compressor",
            "comp.thr",
            "Threshold",
            -40,
            0,
            " dB",
            |m| m.mic_status.compressor.threshold as f32,
            |_, v| G::SetCompressorThreshold(v as i8)
        ),
        slider!(
            MIC,
            2,
            "",
            "comp.makeup",
            "Makeup gain",
            0,
            24,
            " dB",
            |m| m.mic_status.compressor.makeup_gain as f32,
            |_, v| G::SetCompressorMakeupGain(v as i8)
        ),
        choice!(
            MIC,
            2,
            "",
            "comp.ratio",
            "Ratio",
            false,
            CompressorRatio,
            |m: &MixerStatus| m.mic_status.compressor.ratio,
            G::SetCompressorRatio
        ),
        choice!(
            MIC,
            2,
            "",
            "comp.atk",
            "Attack",
            false,
            CompressorAttackTime,
            |m: &MixerStatus| m.mic_status.compressor.attack,
            G::SetCompressorAttack
        ),
        choice!(
            MIC,
            2,
            "",
            "comp.rel",
            "Release",
            false,
            CompressorReleaseTime,
            |m: &MixerStatus| m.mic_status.compressor.release,
            G::SetCompressorReleaseTime
        ),
        choice!(
            FX,
            1,
            "Reverb",
            "fx.rev.style",
            "Style",
            false,
            ReverbStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(ReverbStyle::Library, |e| e.current.reverb.style),
            G::SetReverbStyle
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.amount",
            "Amount",
            0,
            100,
            "%",
            fx!(reverb.amount),
            |_, v| G::SetReverbAmount(v as u8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.decay",
            "Decay",
            10,
            20000,
            " ms",
            fx!(reverb.decay),
            |_, v| G::SetReverbDecay(v as u16)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.early",
            "Early level",
            -25,
            0,
            " dB",
            fx!(reverb.early_level),
            |_, v| G::SetReverbEarlyLevel(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.tail",
            "Tail level",
            -25,
            0,
            " dB",
            fx!(reverb.tail_level),
            |_, v| G::SetReverbTailLevel(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.pre",
            "Pre-delay",
            0,
            100,
            " ms",
            fx!(reverb.pre_delay),
            |_, v| G::SetReverbPreDelay(v as u8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.lo",
            "Low colour",
            -50,
            50,
            "",
            fx!(reverb.lo_colour),
            |_, v| G::SetReverbLowColour(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.hi",
            "High colour",
            -50,
            50,
            "",
            fx!(reverb.hi_colour),
            |_, v| G::SetReverbHighColour(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.hif",
            "High factor",
            -25,
            25,
            "",
            fx!(reverb.hi_factor),
            |_, v| G::SetReverbHighFactor(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.diff",
            "Diffuse",
            -50,
            50,
            "",
            fx!(reverb.diffuse),
            |_, v| G::SetReverbDiffuse(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.mods",
            "Mod speed",
            -25,
            25,
            "",
            fx!(reverb.mod_speed),
            |_, v| G::SetReverbModSpeed(v as i8)
        ),
        slider!(
            FX,
            1,
            "",
            "fx.rev.modd",
            "Mod depth",
            -25,
            25,
            "",
            fx!(reverb.mod_depth),
            |_, v| G::SetReverbModDepth(v as i8)
        ),
        choice!(
            FX,
            2,
            "Echo",
            "fx.echo.style",
            "Style",
            false,
            EchoStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(EchoStyle::Quarter, |e| e.current.echo.style),
            G::SetEchoStyle
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.amount",
            "Amount",
            0,
            100,
            "%",
            fx!(echo.amount),
            |_, v| G::SetEchoAmount(v as u8)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.fb",
            "Feedback",
            0,
            100,
            "%",
            fx!(echo.feedback),
            |_, v| G::SetEchoFeedback(v as u8)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.tempo",
            "Tempo",
            45,
            300,
            " bpm",
            fx!(echo.tempo),
            |_, v| G::SetEchoTempo(v as u16)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.dl",
            "Delay left",
            0,
            2000,
            " ms",
            fx!(echo.delay_left),
            |_, v| G::SetEchoDelayLeft(v as u16)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.dr",
            "Delay right",
            0,
            2000,
            " ms",
            fx!(echo.delay_right),
            |_, v| G::SetEchoDelayRight(v as u16)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.fl",
            "Feedback left",
            0,
            100,
            "%",
            fx!(echo.feedback_left),
            |_, v| G::SetEchoFeedbackLeft(v as u8)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.fr",
            "Feedback right",
            0,
            100,
            "%",
            fx!(echo.feedback_right),
            |_, v| G::SetEchoFeedbackRight(v as u8)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.lr",
            "XFB left to right",
            0,
            100,
            "%",
            fx!(echo.feedback_xfb_l_to_r),
            |_, v| G::SetEchoFeedbackXFBLtoR(v as u8)
        ),
        slider!(
            FX,
            2,
            "",
            "fx.echo.rl",
            "XFB right to left",
            0,
            100,
            "%",
            fx!(echo.feedback_xfb_r_to_l),
            |_, v| G::SetEchoFeedbackXFBRtoL(v as u8)
        ),
        choice!(
            FX,
            3,
            "Pitch",
            "fx.pitch.style",
            "Style",
            true,
            PitchStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(PitchStyle::Narrow, |e| e.current.pitch.style),
            G::SetPitchStyle
        ),
        slider!(
            FX,
            3,
            "",
            "fx.pitch.amount",
            "Amount",
            -24,
            24,
            "",
            fx!(pitch.amount),
            |_, v| G::SetPitchAmount(v as i8)
        ),
        slider!(
            FX,
            3,
            "",
            "fx.pitch.char",
            "Character",
            0,
            100,
            "",
            fx!(pitch.character),
            |_, v| G::SetPitchCharacter(v as u8)
        ),
        choice!(
            FX,
            4,
            "Gender",
            "fx.gender.style",
            "Style",
            true,
            GenderStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(GenderStyle::Narrow, |e| e.current.gender.style),
            G::SetGenderStyle
        ),
        slider!(
            FX,
            4,
            "",
            "fx.gender.amount",
            "Amount",
            -24,
            24,
            "",
            fx!(gender.amount),
            |_, v| G::SetGenderAmount(v as i8)
        ),
        toggle!(
            FX,
            5,
            "Megaphone",
            "fx.mega.on",
            "Enabled",
            fxb!(megaphone.is_enabled),
            |_, v| G::SetMegaphoneEnabled(v)
        ),
        choice!(
            FX,
            5,
            "",
            "fx.mega.style",
            "Style",
            false,
            MegaphoneStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(MegaphoneStyle::Megaphone, |e| e.current.megaphone.style),
            G::SetMegaphoneStyle
        ),
        slider!(
            FX,
            5,
            "",
            "fx.mega.amount",
            "Amount",
            0,
            100,
            "%",
            fx!(megaphone.amount),
            |_, v| G::SetMegaphoneAmount(v as u8)
        ),
        slider!(
            FX,
            5,
            "",
            "fx.mega.gain",
            "Post gain",
            -20,
            20,
            " dB",
            fx!(megaphone.post_gain),
            |_, v| G::SetMegaphonePostGain(v as i8)
        ),
        toggle!(
            FX,
            6,
            "Robot",
            "fx.robot.on",
            "Enabled",
            fxb!(robot.is_enabled),
            |_, v| G::SetRobotEnabled(v)
        ),
        choice!(
            FX,
            6,
            "",
            "fx.robot.style",
            "Style",
            true,
            RobotStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(RobotStyle::Robot1, |e| e.current.robot.style),
            G::SetRobotStyle
        ),
        slider!(
            FX,
            6,
            "Low band",
            "fx.robot.lg",
            "Gain",
            -12,
            12,
            " dB",
            fx!(robot.low_gain),
            |_, v| G::SetRobotGain(RobotRange::Low, v as i8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.lf",
            "Frequency",
            0,
            88,
            "",
            fx!(robot.low_freq),
            |_, v| G::SetRobotFreq(RobotRange::Low, v as u8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.lw",
            "Width",
            0,
            88,
            "",
            fx!(robot.low_width),
            |_, v| G::SetRobotWidth(RobotRange::Low, v as u8)
        ),
        slider!(
            FX,
            6,
            "Medium band",
            "fx.robot.mg",
            "Gain",
            -12,
            12,
            " dB",
            fx!(robot.mid_gain),
            |_, v| G::SetRobotGain(RobotRange::Medium, v as i8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.mf",
            "Frequency",
            0,
            88,
            "",
            fx!(robot.mid_freq),
            |_, v| G::SetRobotFreq(RobotRange::Medium, v as u8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.mw",
            "Width",
            0,
            88,
            "",
            fx!(robot.mid_width),
            |_, v| G::SetRobotWidth(RobotRange::Medium, v as u8)
        ),
        slider!(
            FX,
            6,
            "High band",
            "fx.robot.hg",
            "Gain",
            -12,
            12,
            " dB",
            fx!(robot.high_gain),
            |_, v| G::SetRobotGain(RobotRange::High, v as i8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.hf",
            "Frequency",
            0,
            88,
            "",
            fx!(robot.high_freq),
            |_, v| G::SetRobotFreq(RobotRange::High, v as u8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.hw",
            "Width",
            0,
            88,
            "",
            fx!(robot.high_width),
            |_, v| G::SetRobotWidth(RobotRange::High, v as u8)
        ),
        slider!(
            FX,
            6,
            "Voice",
            "fx.robot.wave",
            "Waveform",
            0,
            2,
            "",
            fx!(robot.waveform),
            |_, v| G::SetRobotWaveform(v as u8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.pulse",
            "Pulse width",
            0,
            100,
            "",
            fx!(robot.pulse_width),
            |_, v| G::SetRobotPulseWidth(v as u8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.thr",
            "Threshold",
            -36,
            0,
            " dB",
            fx!(robot.threshold),
            |_, v| G::SetRobotThreshold(v as i8)
        ),
        slider!(
            FX,
            6,
            "",
            "fx.robot.dry",
            "Dry mix",
            -36,
            0,
            " dB",
            fx!(robot.dry_mix),
            |_, v| G::SetRobotDryMix(v as i8)
        ),
        toggle!(
            FX,
            7,
            "Hard tune",
            "fx.hard.on",
            "Enabled",
            fxb!(hard_tune.is_enabled),
            |_, v| G::SetHardTuneEnabled(v)
        ),
        choice!(
            FX,
            7,
            "",
            "fx.hard.style",
            "Style",
            true,
            HardTuneStyle,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(HardTuneStyle::Natural, |e| e.current.hard_tune.style),
            G::SetHardTuneStyle
        ),
        slider!(
            FX,
            7,
            "",
            "fx.hard.amount",
            "Amount",
            0,
            100,
            "%",
            fx!(hard_tune.amount),
            |_, v| G::SetHardTuneAmount(v as u8)
        ),
        slider!(
            FX,
            7,
            "",
            "fx.hard.rate",
            "Rate",
            0,
            100,
            "",
            fx!(hard_tune.rate),
            |_, v| G::SetHardTuneRate(v as u8)
        ),
        slider!(
            FX,
            7,
            "",
            "fx.hard.window",
            "Window",
            0,
            600,
            " ms",
            fx!(hard_tune.window),
            |_, v| G::SetHardTuneWindow(v as u16)
        ),
        choice!(
            FX,
            7,
            "",
            "fx.hard.source",
            "Source",
            false,
            HardTuneSource,
            |m: &MixerStatus| m
                .effects
                .as_ref()
                .map_or(HardTuneSource::All, |e| e.current.hard_tune.source),
            G::SetHardTuneSource
        ),
        toggle!(
            SETTINGS,
            0,
            "Behaviour",
            "set.vcmute",
            "Voice chat mute also mutes cough",
            |m| m.settings.vc_mute_also_mute_cm,
            |_, v| G::SetVCMuteAlsoMuteCM(v)
        ),
        slider!(
            SETTINGS,
            0,
            "",
            "set.hold",
            "Mute hold time",
            0,
            5000,
            " ms",
            |m| m.settings.mute_hold_duration as f32,
            |_, v| G::SetMuteHoldDuration(v as u16)
        ),
        choice!(
            SETTINGS,
            0,
            "",
            "set.vod",
            "VOD mode",
            true,
            VodMode,
            |m: &MixerStatus| m.settings.vod_mode,
            G::SetVodMode
        ),
    ]
});

pub fn find(key: &str) -> Option<&'static Row> {
    ROWS.iter().find(|r| r.key == key)
}

pub fn build(page: u8, tab: u8, m: &MixerStatus) -> Vec<C> {
    let mut out = vec![];
    for r in ROWS.iter().filter(|r| r.page == page && r.tab == tab) {
        if !r.sec.is_empty() {
            out.push(C::heading(r.sec));
        }
        out.push(match &r.k {
            K::Slider {
                min,
                max,
                unit,
                get,
                ..
            } => C::slider(r.key, r.label, *min, *max, 1.0, get(m), unit),
            K::Toggle { get, .. } => C::toggle(r.key, r.label, get(m)),
            K::Choice { seg, opts, get, .. } => {
                if *seg {
                    C::seg(r.key, r.label, opts(), get(m))
                } else {
                    C::choice(r.key, r.label, opts(), get(m))
                }
            }
        });
    }
    out
}

pub fn command(r: &Row, m: &MixerStatus, value: f32) -> Option<G> {
    match &r.k {
        K::Slider { set, .. } => Some(set(m, value)),
        K::Toggle { set, .. } => Some(set(m, value > 0.5)),
        K::Choice { set, .. } => set(value as usize),
    }
}
