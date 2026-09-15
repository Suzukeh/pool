//! pool render-core: wgpu + WGSL のシーンレンダラ（ヘッドレス可）。
//!
//! M2 スコープ：矩形・楕円・テキスト（ASCII）・ビデオ/画像スタブの合成、
//! 明度・ブラー、グリッド Duplicator、wobble。RGBA8 出力（16F は将来）。

pub mod renderer;
pub mod text;

pub use renderer::{RenderError, Renderer};
pub use text::{TextBitmap, rasterize};

use pool_timeline_model::{Rgba, TimelineObject, Value};

/// Number パラメータ取得。
pub fn num_param(o: &TimelineObject, name: &str, def: f64) -> f64 {
    o.params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| match p.value {
            Value::Number(v) => Some(v),
            Value::Integer(v) => Some(v as f64),
            _ => None,
        })
        .unwrap_or(def)
}

/// Integer パラメータ取得。
pub fn int_param(o: &TimelineObject, name: &str, def: i64) -> i64 {
    o.params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| match p.value {
            Value::Integer(v) => Some(v),
            Value::Number(v) => Some(v as i64),
            _ => None,
        })
        .unwrap_or(def)
}

/// Color パラメータ取得。
pub fn color_param(o: &TimelineObject, name: &str, def: Rgba) -> Rgba {
    o.params
        .iter()
        .find(|p| p.name == name)
        .and_then(|p| match &p.value {
            Value::Color(c) => Some(*c),
            _ => None,
        })
        .unwrap_or(def)
}
