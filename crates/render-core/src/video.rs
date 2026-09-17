//! 動画・静止画フレーム供給（M4 スコープ）。
//!
//! - Video: ffmpeg 外部プロセスで単フレーム抽出＋小規模 LRU。
//!   シーク毎起動のためコマ送り向け。連続再生の高速化は将来課題。
//! - Image: `image` クレートでデコード（png/jpeg 等）。

use pool_ffmpeg_io::MediaInfo;
use std::collections::HashMap;
use std::path::Path;
use std::process::{Command, Stdio};

const CACHE_FRAMES: usize = 8;

pub struct VideoSource {
    pub path: String,
    pub info: MediaInfo,
    cache: HashMap<i64, Vec<u8>>,
}

impl VideoSource {
    pub fn open(path: &str) -> Result<Self, String> {
        let info = pool_ffmpeg_io::probe(Path::new(path)).map_err(|e| e.to_string())?;
        if !info.has_video || info.width == 0 || info.height == 0 {
            return Err("no video stream".to_string());
        }
        Ok(Self {
            path: path.to_string(),
            info,
            cache: HashMap::new(),
        })
    }

    pub fn frame_bytes(&mut self, frame: i64) -> Result<&[u8], String> {
        if !self.cache.contains_key(&frame) {
            let bytes = self.decode(frame)?;
            if self.cache.len() >= CACHE_FRAMES {
                self.cache.clear();
            }
            self.cache.insert(frame, bytes);
        }
        self.cache
            .get(&frame)
            .map(Vec::as_slice)
            .ok_or_else(|| "cache miss".to_string())
    }

    fn decode(&self, frame: i64) -> Result<Vec<u8>, String> {
        let ffmpeg = pool_ffmpeg_io::find_ffmpeg().map_err(|e| e.to_string())?;
        let t = frame.max(0) as f64 * self.info.fps_den as f64 / self.info.fps_num.max(1) as f64;
        let out = Command::new(ffmpeg)
            .args([
                "-y",
                "-v",
                "error",
                "-ss",
                &format!("{t:.3}"),
                "-i",
                &self.path,
                "-frames:v",
                "1",
                "-pix_fmt",
                "rgba",
                "-f",
                "rawvideo",
                "pipe:1",
            ])
            .stdin(Stdio::null())
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(format!(
                "decode failed: {}",
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        let expect = (self.info.width as usize) * (self.info.height as usize) * 4;
        if out.stdout.len() < expect {
            return Err(format!("short frame: {} < {expect}", out.stdout.len()));
        }
        Ok(out.stdout[..expect].to_vec())
    }
}

/// 静止画デコード（RGBA8）。
pub fn decode_still(path: &str) -> Result<(u32, u32, Vec<u8>), String> {
    let img = image::open(path).map_err(|e| e.to_string())?.to_rgba8();
    let (w, h) = (img.width(), img.height());
    Ok((w, h, img.into_raw()))
}
