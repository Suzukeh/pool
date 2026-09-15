//! slangc 呼び出し層：Slang ソース→SPIR-V バイナリ＋ディスクキャッシュ。
//!
//! slangc がなければ全機能は使えない（探索のみ成功扱い）。
//! M4 はコンパイル基盤まで。wgpu での実行結合は M5。

use sha2::{Digest, Sha256};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug)]
pub enum SlangError {
    NotFound,
    Compile(String),
    Io(String),
}

impl fmt::Display for SlangError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => write!(f, "slangc not found (set SLANGC_PATH or PATH)"),
            Self::Compile(e) => write!(f, "slangc failed: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for SlangError {}

/// slangc 探索：SLANGC_PATH 環境変数 → PATH。
pub fn find_slangc() -> Option<PathBuf> {
    if let Some(p) = std::env::var_os("SLANGC_PATH") {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Some(p);
        }
    }
    let name = if cfg!(windows) {
        "slangc.exe"
    } else {
        "slangc"
    };
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths)
            .map(|d| d.join(name))
            .find(|p| p.is_file())
    })
}

pub fn slangc_version(slangc: &Path) -> Result<String, SlangError> {
    let out = Command::new(slangc)
        .arg("-v")
        .output()
        .map_err(|e| SlangError::Io(e.to_string()))?;
    if !out.status.success() {
        return Err(SlangError::Compile("version query failed".to_string()));
    }
    Ok(String::from_utf8_lossy(&out.stdout)
        .lines()
        .next()
        .unwrap_or("")
        .to_string())
}

/// Slang ソースを SPIR-V にコンパイルする。同一内容はキャッシュ再利用。
/// `entry` はエントリーポイント名、`stage` は `fragment` / `compute` 等。
pub fn compile_to_spirv(
    slangc: &Path,
    source: &str,
    entry: &str,
    stage: &str,
    cache_dir: &Path,
) -> Result<PathBuf, SlangError> {
    let mut hasher = Sha256::new();
    hasher.update(source.as_bytes());
    hasher.update(entry.as_bytes());
    hasher.update(stage.as_bytes());
    let hash = hex_of(hasher);
    std::fs::create_dir_all(cache_dir).map_err(|e| SlangError::Io(e.to_string()))?;
    let out = cache_dir.join(format!("{hash}.spv"));
    if out.is_file() {
        return Ok(out);
    }
    let src_path = cache_dir.join(format!("{hash}.slang"));
    std::fs::write(&src_path, source).map_err(|e| SlangError::Io(e.to_string()))?;
    let result = Command::new(slangc)
        .arg(&src_path)
        .arg("-target")
        .arg("spirv")
        .arg("-entry")
        .arg(entry)
        .arg("-stage")
        .arg(stage)
        .arg("-o")
        .arg(&out)
        .output()
        .map_err(|e| SlangError::Io(e.to_string()))?;
    if !result.status.success() {
        let _ = std::fs::remove_file(&out);
        return Err(SlangError::Compile(
            String::from_utf8_lossy(&result.stderr).into_owned(),
        ));
    }
    Ok(out)
}

fn hex_of(hasher: Sha256) -> String {
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const FRAG: &str = r#"
[shader("fragment")]
float4 main(float4 pos: float4) : SV_Target
{
    return float4(1.0, 0.0, 0.0, 1.0);
}
"#;

    #[test]
    fn compile_fragment_when_toolchain_present() {
        let Some(slangc) = find_slangc() else {
            eprintln!("SKIP: slangc not found");
            return;
        };
        let dir = std::env::temp_dir().join(format!("pool-slang-{}", std::process::id()));
        let out = compile_to_spirv(&slangc, FRAG, "main", "fragment", &dir).unwrap();
        let bytes = std::fs::read(&out).unwrap();
        assert!(bytes.len() > 20, "spv too small");
        // SPIR-V マジック
        assert_eq!(&bytes[..4], &[0x03, 0x02, 0x23, 0x07]);
        // 2 回目はキャッシュ
        let out2 = compile_to_spirv(&slangc, FRAG, "main", "fragment", &dir).unwrap();
        assert_eq!(out, out2);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
