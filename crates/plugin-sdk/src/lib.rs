//! pool 別プロセスプラグイン SDK 型。
//!
//! 実行方式：ホストが子プロセスを起動し、stdio で NDJSON プロトコル。
//! フレームは RGBA8 ファイル受け渡し（M4。mmap/DMA-BUF 化は M5）。

use serde::{Deserialize, Serialize};
use std::fmt;

/// プラグインマニフェスト（`manifest.json`）。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub version: String,
    #[serde(default)]
    pub description: String,
    pub kind: PluginKind,
}

impl Manifest {
    pub fn load(json: &str) -> Result<Self, String> {
        let m: Self = serde_json::from_str(json).map_err(|e| e.to_string())?;
        m.validate()?;
        Ok(m)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.name.trim().is_empty() {
            return Err("manifest: empty name".to_string());
        }
        match &self.kind {
            PluginKind::Command { program, .. } if program.trim().is_empty() => {
                Err("manifest: empty program".to_string())
            }
            PluginKind::Command { .. } => Ok(()),
        }
    }
}

/// 実行種別。M4 は外部コマンドのみ。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PluginKind {
    Command {
        program: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

// ---------------------------------------------------------------------------
// プロトコル（stdio NDJSON、1 行 1 メッセージ）
// ---------------------------------------------------------------------------

/// ホスト→プラグイン：初期化。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitMsg {
    pub v: u32,
    #[serde(rename = "type")]
    pub kind: String,
    pub width: u32,
    pub height: u32,
    pub params: serde_json::Value,
}

impl InitMsg {
    pub fn new(width: u32, height: u32, params: serde_json::Value) -> Self {
        Self {
            v: 1,
            kind: "init".to_string(),
            width,
            height,
            params,
        }
    }
}

/// ホスト→プラグイン：描画要求。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderMsg {
    pub v: u32,
    #[serde(rename = "type")]
    pub kind: String,
    pub frame: i64,
    pub input: FrameFile,
    pub output: FrameFile,
    pub params: serde_json::Value,
}

/// RGBA8 フレームファイル（row-major、W*H*4 bytes）。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameFile {
    pub path: String,
    pub size: u64,
}

/// プラグイン→ホスト：応答。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Reply {
    Ready,
    Done,
    Error { message: String },
}

impl fmt::Display for Reply {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ready => write!(f, "ready"),
            Self::Done => write!(f, "done"),
            Self::Error { message } => write!(f, "error: {message}"),
        }
    }
}

pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn manifest_roundtrip() {
        let json = r#"{"name":"hello","version":"0.1.0","kind":{"type":"command","program":"python3","args":["plugin.py"]}}"#;
        let m = Manifest::load(json).unwrap();
        assert_eq!(m.name, "hello");
        assert!(m.validate().is_ok());
    }

    #[test]
    fn manifest_rejects_empty_program() {
        let json = r#"{"name":"x","version":"0.1.0","kind":{"type":"command","program":""}}"#;
        assert!(Manifest::load(json).is_err());
    }

    #[test]
    fn reply_serde() {
        let r: Reply = serde_json::from_str(r#"{"type":"done"}"#).unwrap();
        assert_eq!(r, Reply::Done);
        let s = serde_json::to_string(&Reply::Error {
            message: "boom".to_string(),
        })
        .unwrap();
        assert!(s.contains("boom"));
    }
}
