//! pool のタイムラインデータモデル。
//!
//! AviUtl2 式の「1 レイヤー × 時間区間に複数オブジェクト配置」を表現する。
//! UI・GPU に依存しない純粋データ + JSON 永続化が責務。描画・再生は別クレート。

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// プロジェクト JSON のスキーマ版。破壊的変更時は +1 する。
pub const PROJECT_VERSION: u32 = 1;

/// フレーム番号。時刻の唯一の単位（秒は `fps` から導出）。
pub type Frame = i64;

// ---------------------------------------------------------------------------
// 基本型
// ---------------------------------------------------------------------------

/// RGBA カラー（0.0〜1.0、乗算済み α 前提）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Rgba {
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
}

/// パラメータ値。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "snake_case")]
pub enum Value {
    Number(f64),
    Integer(i64),
    Boolean(bool),
    Text(String),
    Color(Rgba),
    Point([f64; 2]),
}

/// キーフレーム補間。Bezier は開始キー側のセグメントに適用される
/// cubic-bezier（CSS 式、時間正規化）。
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Interpolation {
    Hold,
    Linear,
    Smooth,
    Bezier { x1: f64, y1: f64, x2: f64, y2: f64 },
}

impl Interpolation {
    pub fn easy_ease() -> Self {
        Self::Bezier {
            x1: 0.42,
            y1: 0.0,
            x2: 0.58,
            y2: 1.0,
        }
    }

    pub fn ease() -> Self {
        Self::Bezier {
            x1: 0.25,
            y1: 0.1,
            x2: 0.25,
            y2: 1.0,
        }
    }
}

/// 範囲外フレームの振る舞い（パラメータ単位）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoopMode {
    Off,
    Loop,
    PingPong,
}

/// 名前付きパラメータ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Param {
    pub name: String,
    pub value: Value,
}

/// 単一パラメータのキーフレーム。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Keyframe {
    pub param: String,
    pub frame: Frame,
    pub value: Value,
    pub interpolation: Interpolation,
}

// ---------------------------------------------------------------------------
// オブジェクト
// ---------------------------------------------------------------------------

/// 図形種別（MVP 最小）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ShapeKind {
    Rectangle,
    Ellipse,
}

/// タイムライン上のオブジェクト種別。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ObjectKind {
    Video {
        path: String,
    },
    Image {
        path: String,
    },
    Audio {
        path: String,
    },
    Text {
        body: String,
    },
    Shape {
        shape: ShapeKind,
    },
    /// 他オブジェクトを複製配置するプロシージャル源。`source` は参照先 ID。
    Duplicator {
        source: String,
    },
    /// 直前のメディアオブジェクトにかかるフィルタ。
    Filter {
        effect: String,
    },
    /// 下位 N レイヤーを束ねる制御オブジェクト。
    GroupControl {
        target_layers: u32,
    },
    CameraControl {
        target_layers: u32,
    },
}

/// レイヤー上の区間オブジェクト。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimelineObject {
    pub id: String,
    pub name: String,
    pub kind: ObjectKind,
    /// 開始フレーム（含む）。
    pub start_frame: Frame,
    /// 終了フレーム（含まない）。`start_frame < end_frame` 必須。
    pub end_frame: Frame,
    pub params: Vec<Param>,
    pub keyframes: Vec<Keyframe>,
    /// パラメータ名 → 範囲外の振る舞い（既定 Off）。
    #[serde(default)]
    pub loops: HashMap<String, LoopMode>,
}

impl TimelineObject {
    /// 区間長（フレーム数）。
    pub fn len_frames(&self) -> Frame {
        self.end_frame - self.start_frame
    }

    /// 空区間か。
    pub fn is_empty(&self) -> bool {
        self.end_frame <= self.start_frame
    }

    /// フレームを含むか。
    pub fn contains(&self, frame: Frame) -> bool {
        self.start_frame <= frame && frame < self.end_frame
    }

    /// パラメータ参照。
    pub fn param(&self, name: &str) -> Option<&Value> {
        self.params
            .iter()
            .find(|p| p.name == name)
            .map(|p| &p.value)
    }
}

// ---------------------------------------------------------------------------
// 評価（キーフレーム→値）
// ---------------------------------------------------------------------------

fn cubic_bezier(t: f64, x1: f64, y1: f64, x2: f64, y2: f64) -> f64 {
    let t = t.clamp(0.0, 1.0);
    let (mut lo, mut hi) = (0.0, 1.0);
    let mut s = t;
    for _ in 0..12 {
        s = (lo + hi) / 2.0;
        let x = 3.0 * (1.0 - s).powi(2) * s * x1 + 3.0 * (1.0 - s) * s * s * x2 + s * s * s;
        if x < t {
            lo = s;
        } else {
            hi = s;
        }
    }
    let y = 3.0 * (1.0 - s).powi(2) * s * y1 + 3.0 * (1.0 - s) * s * s * y2 + s * s * s;
    y.clamp(0.0, 1.0)
}

fn lerp_value(a: &Value, b: &Value, t: f64) -> Value {
    match (a, b) {
        (Value::Number(x), Value::Number(y)) => Value::Number(x + (y - x) * t),
        (Value::Integer(x), Value::Integer(y)) => {
            Value::Integer((*x as f64 + (*y as f64 - *x as f64) * t).round() as i64)
        }
        (Value::Color(x), Value::Color(y)) => Value::Color(Rgba {
            r: (x.r as f64 + (y.r as f64 - x.r as f64) * t) as f32,
            g: (x.g as f64 + (y.g as f64 - x.g as f64) * t) as f32,
            b: (x.b as f64 + (y.b as f64 - x.b as f64) * t) as f32,
            a: (x.a as f64 + (y.a as f64 - x.a as f64) * t) as f32,
        }),
        (Value::Point(x), Value::Point(y)) => {
            Value::Point([x[0] + (y[0] - x[0]) * t, x[1] + (y[1] - x[1]) * t])
        }
        // Boolean / Text / 型不一致は hold
        _ => a.clone(),
    }
}

impl TimelineObject {
    pub fn loop_mode(&self, param: &str) -> LoopMode {
        self.loops.get(param).copied().unwrap_or(LoopMode::Off)
    }

    pub fn has_keys(&self, param: &str) -> bool {
        self.keyframes.iter().any(|k| k.param == param)
    }

    pub fn keyframes_for(&self, param: &str) -> Vec<&Keyframe> {
        let mut v: Vec<&Keyframe> = self.keyframes.iter().filter(|k| k.param == param).collect();
        v.sort_by_key(|k| k.frame);
        v
    }

    fn map_frame(&self, param: &str, frame: Frame, first: Frame, last: Frame) -> Frame {
        let period = last - first;
        if period <= 0 {
            return first;
        }
        match self.loop_mode(param) {
            LoopMode::Off => frame.clamp(first, last),
            LoopMode::Loop => first + (frame - first).rem_euclid(period),
            LoopMode::PingPong => {
                let m = (frame - first).rem_euclid(period * 2);
                if m <= period {
                    first + m
                } else {
                    last - (m - period)
                }
            }
        }
    }

    /// パラメータ評価。キーなし→基本値→def の順でフォールバック。
    pub fn eval_value(&self, param: &str, frame: Frame, def: &Value) -> Value {
        let base = self
            .params
            .iter()
            .find(|p| p.name == param)
            .map(|p| p.value.clone());
        let keys = self.keyframes_for(param);
        if keys.is_empty() {
            return base.unwrap_or_else(|| def.clone());
        }
        let (first, last) = (keys[0].frame, keys[keys.len() - 1].frame);
        let f = self.map_frame(param, frame, first, last);
        let mut idx = 0;
        while idx + 1 < keys.len() && keys[idx + 1].frame <= f {
            idx += 1;
        }
        if idx + 1 >= keys.len() {
            return keys[idx].value.clone();
        }
        let (k0, k1) = (keys[idx], keys[idx + 1]);
        if k1.frame <= k0.frame {
            return k1.value.clone();
        }
        let t = (f - k0.frame) as f64 / (k1.frame - k0.frame) as f64;
        let e = match k0.interpolation {
            Interpolation::Hold => return k0.value.clone(),
            Interpolation::Linear => t,
            Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
            Interpolation::Bezier { x1, y1, x2, y2 } => cubic_bezier(t, x1, y1, x2, y2),
        };
        lerp_value(&k0.value, &k1.value, e.clamp(0.0, 1.0))
    }

    pub fn eval_number(&self, param: &str, frame: Frame, def: f64) -> f64 {
        match self.eval_value(param, frame, &Value::Number(def)) {
            Value::Number(v) => v,
            Value::Integer(v) => v as f64,
            _ => def,
        }
    }

    pub fn eval_int(&self, param: &str, frame: Frame, def: i64) -> i64 {
        match self.eval_value(param, frame, &Value::Integer(def)) {
            Value::Integer(v) => v,
            Value::Number(v) => v.round() as i64,
            _ => def,
        }
    }

    pub fn eval_color(&self, param: &str, frame: Frame, def: Rgba) -> Rgba {
        match self.eval_value(param, frame, &Value::Color(def)) {
            Value::Color(c) => c,
            _ => def,
        }
    }
}

// ---------------------------------------------------------------------------
// レイヤー・シーン・プロジェクト
// ---------------------------------------------------------------------------

/// レイヤー。`objects` の index 0 が最上位、下に行くほど手前（AviUtl2 踏襲）。
/// 同一レイヤー内のオブジェクトは時間的に重なってはならない。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Layer {
    pub id: String,
    pub name: String,
    #[serde(default = "default_true")]
    pub visible: bool,
    #[serde(default)]
    pub locked: bool,
    #[serde(default)]
    pub objects: Vec<TimelineObject>,
}

fn default_true() -> bool {
    true
}

impl Layer {
    /// 区間の空チェックと時間重なりを検証する。
    pub fn validate(&self) -> Result<(), ModelError> {
        let mut spans: Vec<(&str, Frame, Frame)> = self
            .objects
            .iter()
            .map(|o| (o.id.as_str(), o.start_frame, o.end_frame))
            .collect();
        for (id, s, e) in &spans {
            if e <= s {
                return Err(ModelError::EmptyRange {
                    layer: self.id.clone(),
                    object: (*id).to_string(),
                });
            }
        }
        spans.sort_by_key(|(_, s, _)| *s);
        for w in spans.windows(2) {
            let (a_id, _, a_end) = w[0];
            let (b_id, b_start, _) = w[1];
            if b_start < a_end {
                return Err(ModelError::Overlap {
                    layer: self.id.clone(),
                    first: a_id.to_string(),
                    second: b_id.to_string(),
                });
            }
        }
        Ok(())
    }
}

/// シーン（AE の Comp 相当）。解像度・FPS を持つ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Scene {
    pub id: String,
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub sample_rate: u32,
    pub bg_color: Rgba,
    #[serde(default)]
    pub layers: Vec<Layer>,
}

impl Scene {
    /// シーン長＝全オブジェクトの最大 end（空なら 0）。
    pub fn duration_frames(&self) -> Frame {
        self.layers
            .iter()
            .flat_map(|l| l.objects.iter().map(|o| o.end_frame))
            .max()
            .unwrap_or(0)
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.fps_den == 0 {
            return Err(ModelError::InvalidFps {
                scene: self.id.clone(),
            });
        }
        for layer in &self.layers {
            layer.validate()?;
        }
        Ok(())
    }

    /// 秒→フレーム（四捨五入）。
    pub fn seconds_to_frame(&self, seconds: f64) -> Frame {
        (seconds * self.fps_num as f64 / self.fps_den as f64).round() as Frame
    }
}

/// プロジェクト。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Project {
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub scenes: Vec<Scene>,
    pub active_scene_id: String,
}

impl Project {
    pub fn new(name: impl Into<String>) -> Self {
        let scene = Scene {
            id: "scene-root".to_string(),
            name: "Root".to_string(),
            width: 1920,
            height: 1080,
            fps_num: 30,
            fps_den: 1,
            sample_rate: 44100,
            bg_color: Rgba::BLACK,
            layers: vec![Layer {
                id: "layer-1".to_string(),
                name: "Layer1".to_string(),
                visible: true,
                locked: false,
                objects: Vec::new(),
            }],
        };
        let active = scene.id.clone();
        Self {
            version: PROJECT_VERSION,
            name: name.into(),
            scenes: vec![scene],
            active_scene_id: active,
        }
    }

    pub fn active_scene(&self) -> Option<&Scene> {
        self.scenes.iter().find(|s| s.id == self.active_scene_id)
    }

    pub fn validate(&self) -> Result<(), ModelError> {
        if self.scenes.is_empty() {
            return Err(ModelError::EmptyScenes);
        }
        if self.active_scene().is_none() {
            return Err(ModelError::SceneNotFound {
                id: self.active_scene_id.clone(),
            });
        }
        for scene in &self.scenes {
            scene.validate()?;
        }
        Ok(())
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, ModelError> {
        let project: Self = serde_json::from_str(json)?;
        if project.version != PROJECT_VERSION {
            return Err(ModelError::UnsupportedVersion {
                found: project.version,
            });
        }
        project.validate()?;
        Ok(project)
    }
}

// ---------------------------------------------------------------------------
// エラー
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
pub enum ModelError {
    Json(String),
    UnsupportedVersion {
        found: u32,
    },
    EmptyScenes,
    SceneNotFound {
        id: String,
    },
    InvalidFps {
        scene: String,
    },
    EmptyRange {
        layer: String,
        object: String,
    },
    Overlap {
        layer: String,
        first: String,
        second: String,
    },
}

impl fmt::Display for ModelError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::UnsupportedVersion { found } => {
                write!(
                    f,
                    "unsupported project version {found} (expected {PROJECT_VERSION})"
                )
            }
            Self::EmptyScenes => write!(f, "project has no scenes"),
            Self::SceneNotFound { id } => write!(f, "active scene not found: {id}"),
            Self::InvalidFps { scene } => write!(f, "scene has zero fps denominator: {scene}"),
            Self::EmptyRange { layer, object } => {
                write!(f, "object has empty range: {layer}/{object}")
            }
            Self::Overlap {
                layer,
                first,
                second,
            } => {
                write!(f, "objects overlap in {layer}: {first} vs {second}")
            }
        }
    }
}

impl std::error::Error for ModelError {}

impl From<serde_json::Error> for ModelError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e.to_string())
    }
}

// ---------------------------------------------------------------------------
// テスト
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_object(id: &str, start: Frame, end: Frame) -> TimelineObject {
        TimelineObject {
            id: id.to_string(),
            name: id.to_string(),
            kind: ObjectKind::Text {
                body: "hello".to_string(),
            },
            start_frame: start,
            end_frame: end,
            params: vec![Param {
                name: "x".to_string(),
                value: Value::Number(10.0),
            }],
            keyframes: Vec::new(),
            loops: Default::default(),
        }
    }

    #[test]
    fn new_project_is_valid() {
        let p = Project::new("demo");
        assert_eq!(p.version, PROJECT_VERSION);
        assert!(p.validate().is_ok());
        assert_eq!(p.active_scene().unwrap().name, "Root");
    }

    #[test]
    fn roundtrip_json() {
        let mut p = Project::new("demo");
        let scene = &mut p.scenes[0];
        scene.layers[0].objects.push(sample_object("o1", 0, 90));
        scene.layers[0].objects.push(sample_object("o2", 90, 150));
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn rejects_overlap_in_layer() {
        let mut layer = Layer {
            id: "l".to_string(),
            name: "L".to_string(),
            visible: true,
            locked: false,
            objects: vec![sample_object("a", 0, 100), sample_object("b", 50, 120)],
        };
        assert!(matches!(layer.validate(), Err(ModelError::Overlap { .. })));
        layer.objects[1].start_frame = 100;
        assert!(layer.validate().is_ok());
    }

    #[test]
    fn rejects_empty_range() {
        let layer = Layer {
            id: "l".to_string(),
            name: "L".to_string(),
            visible: true,
            locked: false,
            objects: vec![sample_object("a", 30, 30)],
        };
        assert!(matches!(
            layer.validate(),
            Err(ModelError::EmptyRange { .. })
        ));
    }

    #[test]
    fn duration_is_max_end() {
        let mut p = Project::new("demo");
        p.scenes[0].layers[0]
            .objects
            .push(sample_object("a", 0, 90));
        p.scenes[0].layers[0]
            .objects
            .push(sample_object("b", 100, 301));
        assert_eq!(p.scenes[0].duration_frames(), 301);
    }

    #[test]
    fn rejects_unknown_version() {
        let json = r#"{"version":999,"name":"x","scenes":[],"active_scene_id":"s"}"#;
        assert!(matches!(
            Project::from_json(json),
            Err(ModelError::UnsupportedVersion { found: 999 })
        ));
    }

    #[test]
    fn seconds_to_frame_rounds() {
        let p = Project::new("demo");
        let scene = &p.scenes[0]; // 30fps
        assert_eq!(scene.seconds_to_frame(10.0), 300);
        assert_eq!(scene.seconds_to_frame(0.05), 2); // 1.5 -> 2
    }

    fn keyed_object() -> TimelineObject {
        TimelineObject {
            id: "k".to_string(),
            name: "k".to_string(),
            kind: ObjectKind::Text {
                body: "x".to_string(),
            },
            start_frame: 0,
            end_frame: 100,
            params: vec![],
            keyframes: vec![
                Keyframe {
                    param: "x".to_string(),
                    frame: 0,
                    value: Value::Number(0.0),
                    interpolation: Interpolation::Linear,
                },
                Keyframe {
                    param: "x".to_string(),
                    frame: 10,
                    value: Value::Number(10.0),
                    interpolation: Interpolation::Linear,
                },
            ],
            loops: Default::default(),
        }
    }

    #[test]
    fn eval_linear_midpoint() {
        let o = keyed_object();
        assert!((o.eval_number("x", 5, -1.0) - 5.0).abs() < 1e-9);
        assert!((o.eval_number("x", 0, -1.0) - 0.0).abs() < 1e-9);
        assert!((o.eval_number("x", 10, -1.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn eval_clamps_without_loop() {
        let o = keyed_object();
        assert!((o.eval_number("x", -5, -1.0) - 0.0).abs() < 1e-9);
        assert!((o.eval_number("x", 99, -1.0) - 10.0).abs() < 1e-9);
    }

    #[test]
    fn eval_loop_wraps() {
        let mut o = keyed_object();
        o.loops.insert("x".to_string(), LoopMode::Loop);
        assert!((o.eval_number("x", 25, -1.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn eval_pingpong_mirrors() {
        let mut o = keyed_object();
        o.loops.insert("x".to_string(), LoopMode::PingPong);
        assert!((o.eval_number("x", 15, -1.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn eval_bezier_identity_matches_linear() {
        let mut o = keyed_object();
        o.keyframes[0].interpolation = Interpolation::Bezier {
            x1: 0.0,
            y1: 0.0,
            x2: 1.0,
            y2: 1.0,
        };
        assert!((o.eval_number("x", 3, -1.0) - 3.0).abs() < 0.05);
    }

    #[test]
    fn eval_smooth_midpoint_is_half() {
        let mut o = keyed_object();
        o.keyframes[0].interpolation = Interpolation::Smooth;
        assert!((o.eval_number("x", 5, -1.0) - 5.0).abs() < 1e-9);
    }

    #[test]
    fn eval_hold_keeps_start() {
        let mut o = keyed_object();
        o.keyframes[0].interpolation = Interpolation::Hold;
        assert!((o.eval_number("x", 5, -1.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn eval_missing_param_returns_default() {
        let o = keyed_object();
        assert!((o.eval_number("nope", 5, 42.0) - 42.0).abs() < 1e-9);
    }

    #[test]
    fn eval_prefers_base_param_without_keys() {
        let mut o = keyed_object();
        o.keyframes.clear();
        o.params.push(Param {
            name: "x".to_string(),
            value: Value::Number(3.0),
        });
        assert!((o.eval_number("x", 5, 42.0) - 3.0).abs() < 1e-9);
    }
}
