//! AviUtl スクリプト（.anm / .obj / .cam / .scn / .tra と 2 系）の読込。
//!
//! - 文字コード：UTF-8（BOM 可）→ 失敗時は Shift_JIS。
//! - `@名前` で複数エフェクト同梱に対応。
//! - 実行はしない。ダイアログ定義を pool パラメータに写す（将来の実行エンジン用）。

use pool_timeline_model::{Frame, Param, TimelineObject, Value};
use serde::{Deserialize, Serialize};

/// スクリプト種別（拡張子から判定）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScriptKind {
    Anim,
    Obj,
    Cam,
    Scn,
    Tra,
    Unknown,
}

impl ScriptKind {
    pub fn from_extension(ext: &str) -> Self {
        let lower = ext.to_ascii_lowercase();
        match lower.trim_start_matches('.') {
            "anm" | "anm2" => Self::Anim,
            "obj" | "obj2" => Self::Obj,
            "cam" | "cam2" => Self::Cam,
            "scn" | "scn2" => Self::Scn,
            "tra" | "tra2" => Self::Tra,
            _ => Self::Unknown,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Track {
    pub index: u32,
    pub name: String,
    pub min: f64,
    pub max: f64,
    pub def: f64,
    pub step: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Check {
    pub index: u32,
    pub name: String,
    pub def: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dialog {
    pub text: String,
    pub def: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ColorDef {
    pub name: String,
    pub def: String,
}

/// 1 エフェクト分。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Effect {
    pub name: String,
    pub tracks: Vec<Track>,
    pub checks: Vec<Check>,
    pub dialogs: Vec<Dialog>,
    pub files: Vec<String>,
    pub colors: Vec<ColorDef>,
    pub params: Vec<String>,
    pub anchors: Vec<String>,
    pub script_lang: Option<String>,
    /// 解釈しなかった -- 行（ロスレス保持）。
    pub raw_headers: Vec<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AviScript {
    pub kind: ScriptKind,
    pub effects: Vec<Effect>,
}

fn parse_num(s: &str, def: f64) -> f64 {
    s.trim().parse().unwrap_or(def)
}

fn parse_track(index: u32, rest: &str) -> Track {
    // name,min,max[,def][,step]
    let parts: Vec<&str> = rest.split(',').collect();
    let name = parts.first().unwrap_or(&"").trim().to_string();
    let min = parts.get(1).map(|s| parse_num(s, 0.0)).unwrap_or(0.0);
    let max = parts.get(2).map(|s| parse_num(s, 100.0)).unwrap_or(100.0);
    let def = parts.get(3).map(|s| parse_num(s, min)).unwrap_or(min);
    let step = parts.get(4).map(|s| parse_num(s, 1.0));
    Track {
        index,
        name,
        min,
        max,
        def,
        step,
    }
}

fn parse_check(index: u32, rest: &str) -> Check {
    let mut it = rest.splitn(2, ',');
    let name = it.next().unwrap_or("").trim().to_string();
    let def = it
        .next()
        .map(|s| matches!(s.trim(), "1" | "true" | "on"))
        .unwrap_or(false);
    Check { index, name, def }
}

fn parse_dialog(rest: &str) -> Dialog {
    let mut it = rest.splitn(2, ',');
    Dialog {
        text: it.next().unwrap_or("").trim().to_string(),
        def: it.next().unwrap_or("").trim().to_string(),
    }
}

fn apply_header(fx: &mut Effect, key: &str, rest: &str) {
    if let Some(idx) = key
        .strip_prefix("track")
        .and_then(|n| n.parse::<u32>().ok())
    {
        fx.tracks.push(parse_track(idx, rest));
    } else if let Some(idx) = key
        .strip_prefix("check")
        .and_then(|n| n.parse::<u32>().ok())
    {
        fx.checks.push(parse_check(idx, rest));
    } else {
        match key {
            "dialog" => fx.dialogs.push(parse_dialog(rest)),
            "file" => fx.files.push(rest.trim().to_string()),
            "color" => {
                let mut it = rest.splitn(2, ',');
                fx.colors.push(ColorDef {
                    name: it.next().unwrap_or("").trim().to_string(),
                    def: it.next().unwrap_or("").trim().to_string(),
                });
            }
            "param" => fx.params.push(rest.trim().to_string()),
            "anchor" => fx.anchors.push(rest.trim().to_string()),
            "script" => fx.script_lang = Some(rest.trim().to_string()),
            _ => fx.raw_headers.push(format!("--{key}:{rest}")),
        }
    }
}

fn parse_section(name: String, lines: &[&str]) -> Effect {
    let mut fx = Effect {
        name,
        tracks: vec![],
        checks: vec![],
        dialogs: vec![],
        files: vec![],
        colors: vec![],
        params: vec![],
        anchors: vec![],
        script_lang: None,
        raw_headers: vec![],
        body: String::new(),
    };
    let mut body_start = lines.len();
    for (i, line) in lines.iter().enumerate() {
        let t = line.trim_start();
        if !t.starts_with("--") {
            body_start = i;
            break;
        }
        let inner = t[2..].trim();
        match inner.find(':') {
            Some(p) => {
                let (k, v) = inner.split_at(p);
                apply_header(&mut fx, k.trim(), v[1..].trim());
            }
            None => fx.raw_headers.push(t.to_string()),
        }
    }
    fx.body = lines[body_start..].join("\n");
    fx.tracks.sort_by_key(|t| t.index);
    fx.checks.sort_by_key(|c| c.index);
    fx
}

fn decode_bytes(bytes: &[u8]) -> String {
    if let Ok(s) = std::str::from_utf8(bytes) {
        return s.strip_prefix('\u{FEFF}').unwrap_or(s).to_string();
    }
    let (s, _, _) = encoding_rs::SHIFT_JIS.decode(bytes);
    s.into_owned()
}

/// バイト列＋拡張子から読む。
pub fn parse_bytes(bytes: &[u8], extension: &str, fallback_name: &str) -> AviScript {
    let text = decode_bytes(bytes);
    parse_text(&text, extension, fallback_name)
}

/// テキストから読む。先頭 `@` がなければ単一エフェクト（名＝ファイル名）。
pub fn parse_text(text: &str, extension: &str, fallback_name: &str) -> AviScript {
    let lines: Vec<&str> = text.lines().collect();
    let mut sections: Vec<(String, Vec<&str>)> = Vec::new();
    let mut current: Option<(String, Vec<&str>)> = None;
    for line in lines {
        let t = line.trim_start();
        if let Some(name) = t.strip_prefix('@') {
            if let Some(sec) = current.take() {
                sections.push(sec);
            }
            current = Some((name.trim().to_string(), Vec::new()));
        } else {
            if current.is_none() {
                current = Some((fallback_name.to_string(), Vec::new()));
            }
            current.as_mut().unwrap().1.push(line);
        }
    }
    if let Some(sec) = current.take() {
        sections.push(sec);
    }
    AviScript {
        kind: ScriptKind::from_extension(extension),
        effects: sections
            .into_iter()
            .map(|(n, l)| parse_section(n, &l))
            .collect(),
    }
}

impl Effect {
    /// pool Filter オブジェクト化（実行は将来。定義のロスレス保持が目的）。
    pub fn to_pool_filter(&self, id: &str, start: Frame, end: Frame) -> TimelineObject {
        let mut params = vec![
            Param {
                name: "script_name".to_string(),
                value: Value::Text(self.name.clone()),
            },
            Param {
                name: "script_body".to_string(),
                value: Value::Text(self.body.clone()),
            },
        ];
        for t in &self.tracks {
            params.push(Param {
                name: format!("track{}", t.index),
                value: Value::Number(t.def),
            });
        }
        for c in &self.checks {
            params.push(Param {
                name: format!("check{}", c.index),
                value: Value::Boolean(c.def),
            });
        }
        for (i, d) in self.dialogs.iter().enumerate() {
            params.push(Param {
                name: format!("dialog{i}"),
                value: Value::Text(if d.def.is_empty() {
                    d.text.clone()
                } else {
                    d.def.clone()
                }),
            });
        }
        for (i, f) in self.files.iter().enumerate() {
            params.push(Param {
                name: format!("file{i}"),
                value: Value::Text(f.clone()),
            });
        }
        for (i, c) in self.colors.iter().enumerate() {
            params.push(Param {
                name: format!("color{i}"),
                value: Value::Text(format!("{}={}", c.name, c.def)),
            });
        }
        TimelineObject {
            id: id.to_string(),
            name: self.name.clone(),
            kind: pool_timeline_model::ObjectKind::Filter {
                effect: "lua-script".to_string(),
            },
            start_frame: start,
            end_frame: end,
            params,
            keyframes: vec![],
            loops: Default::default(),
        }
    }
}
