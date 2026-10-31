use crate::Ctl;
use slint::{Color, Model, ModelRc, SharedString, VecModel};
use std::fmt::Display;
use strum::IntoEnumIterator;

#[derive(Clone, PartialEq, Debug)]
pub struct C {
    pub kind: i32,
    pub label: String,
    pub key: String,
    pub min: f32,
    pub max: f32,
    pub step: f32,
    pub value: f32,
    pub checked: bool,
    pub unit: String,
    pub text: String,
    pub options: Vec<String>,
    pub index: i32,
    pub colours: Vec<(u8, u8, u8)>,
    pub enabled: bool,
}

impl C {
    fn base(kind: i32, key: &str, label: &str) -> C {
        C {
            kind,
            label: label.into(),
            key: key.into(),
            min: 0.0,
            max: 0.0,
            step: 1.0,
            value: 0.0,
            checked: false,
            unit: String::new(),
            text: String::new(),
            options: vec![],
            index: -1,
            colours: vec![],
            enabled: true,
        }
    }

    pub fn slider(
        key: &str,
        label: &str,
        min: f32,
        max: f32,
        step: f32,
        value: f32,
        unit: &str,
    ) -> C {
        C {
            min,
            max,
            step,
            value,
            unit: unit.into(),
            ..C::base(0, key, label)
        }
    }

    pub fn slider_link(
        key: &str,
        label: &str,
        min: f32,
        max: f32,
        value: f32,
        link_key: &str,
        linked: bool,
    ) -> C {
        C {
            min,
            max,
            value,
            text: link_key.into(),
            checked: linked,
            ..C::base(11, key, label)
        }
    }

    pub fn toggle(key: &str, label: &str, on: bool) -> C {
        C {
            checked: on,
            ..C::base(1, key, label)
        }
    }

    pub fn choice(key: &str, label: &str, options: Vec<String>, index: i32) -> C {
        C {
            options,
            index,
            ..C::base(2, key, label)
        }
    }

    pub fn seg(key: &str, label: &str, options: Vec<String>, index: i32) -> C {
        C {
            options,
            index,
            ..C::base(3, key, label)
        }
    }

    pub fn heading(label: &str) -> C {
        C::base(4, "", label)
    }

    pub fn colours(key: &str, label: &str, colours: Vec<(u8, u8, u8)>) -> C {
        C {
            colours,
            ..C::base(5, key, label)
        }
    }

    pub fn field(key: &str, label: &str, text: &str) -> C {
        C {
            text: text.into(),
            unit: "Type here, press Enter".into(),
            ..C::base(6, key, label)
        }
    }

    pub fn button(key: &str, label: &str, text: &str, danger: bool) -> C {
        C {
            text: text.into(),
            checked: danger,
            ..C::base(7, key, label)
        }
    }

    pub fn info(label: &str, text: &str) -> C {
        C {
            text: text.into(),
            ..C::base(8, "", label)
        }
    }

    pub fn note(text: &str) -> C {
        C::base(9, "", text)
    }

    pub fn item(key: &str, label: &str, active: bool, load_enabled: bool, buttons: [&str; 2]) -> C {
        C {
            checked: active,
            enabled: load_enabled,
            options: buttons.iter().map(|s| s.to_string()).collect(),
            ..C::base(10, key, label)
        }
    }

    pub fn to_ctl(&self) -> Ctl {
        Ctl {
            kind: self.kind,
            label: SharedString::from(self.label.as_str()),
            key: SharedString::from(self.key.as_str()),
            min: self.min,
            max: self.max,
            step: self.step,
            value: self.value,
            checked: self.checked,
            unit: SharedString::from(self.unit.as_str()),
            text: SharedString::from(self.text.as_str()),
            options: ModelRc::new(VecModel::from(
                self.options
                    .iter()
                    .map(|s| SharedString::from(s.as_str()))
                    .collect::<Vec<_>>(),
            )),
            index: self.index,
            colours: ModelRc::new(VecModel::from(
                self.colours
                    .iter()
                    .map(|c| Color::from_rgb_u8(c.0, c.1, c.2))
                    .collect::<Vec<_>>(),
            )),
            enabled: self.enabled,
        }
    }
}

pub fn pretty(s: &str) -> String {
    if let Some(r) = s.strip_prefix("Ratio") {
        return format!("{}:1", r.replace('_', "."));
    }
    for p in ["Gate", "Comp", "Equalizer"] {
        if let Some(r) = s.strip_prefix(p)
            && r.chars().next().is_some_and(|c| c.is_ascii_digit())
        {
            let n: String = r.chars().take_while(|c| c.is_ascii_digit()).collect();
            let u = r[n.len()..].replace("KHz", "kHz");
            return format!("{} {}", n, u);
        }
    }
    let mut out = String::new();
    let mut prev: Option<char> = None;
    for c in s.chars() {
        if let Some(p) = prev {
            let split = (c.is_uppercase() && (p.is_lowercase() || p.is_ascii_digit()))
                || (c.is_ascii_digit() && p.is_alphabetic());
            if split {
                out.push(' ');
            }
        }
        out.push(c);
        prev = Some(c);
    }
    out
}

pub fn opts<T: IntoEnumIterator + Display>() -> Vec<String> {
    T::iter().map(|v| pretty(&v.to_string())).collect()
}

pub fn idx<T: IntoEnumIterator + PartialEq>(v: &T) -> i32 {
    T::iter().position(|x| x == *v).map_or(-1, |i| i as i32)
}

pub fn nth<T: IntoEnumIterator>(i: usize) -> Option<T> {
    T::iter().nth(i)
}

pub fn hex_to_rgb(s: &str) -> (u8, u8, u8) {
    let s = s.trim_start_matches('#');
    let p = |r: std::ops::Range<usize>| {
        s.get(r)
            .and_then(|x| u8::from_str_radix(x, 16).ok())
            .unwrap_or(0)
    };
    (p(0..2), p(2..4), p(4..6))
}

pub fn packed_to_hex(v: f32) -> String {
    let n = v as u32;
    format!(
        "{:02X}{:02X}{:02X}",
        (n >> 16) & 0xff,
        (n >> 8) & 0xff,
        n & 0xff
    )
}

pub fn sync(model: &VecModel<Ctl>, prev: &mut Vec<C>, new: Vec<C>, skip: &dyn Fn(&str) -> bool) {
    let same_shape = prev.len() == new.len()
        && prev
            .iter()
            .zip(&new)
            .all(|(a, b)| a.key == b.key && a.kind == b.kind);
    if !same_shape {
        model.set_vec(new.iter().map(C::to_ctl).collect::<Vec<_>>());
        *prev = new;
        return;
    }
    for (i, row) in new.into_iter().enumerate() {
        if prev[i] != row && !(skip(&row.key) && !row.key.is_empty()) {
            model.set_row_data(i, row.to_ctl());
            prev[i] = row;
        }
    }
}
