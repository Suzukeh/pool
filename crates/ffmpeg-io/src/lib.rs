//! ffmpeg 外部プロセス連携。libav* へのリンクはしない
//! （別プロセス起動のみ＝LGPL 汚染なし）。
//!
//! - `probe`: ffprobe JSON でメディア情報
//! - `Mp4Writer`: rawvideo パイプで mp4 書出し
//! - `thumbnail`: 先頭フレーム PNG 切り出し

use serde::{Deserialize, Serialize};
use std::fmt;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};

#[derive(Debug)]
pub enum FfmpegError {
    NotFound(&'static str),
    Probe(String),
    Spawn(String),
    Write(String),
    Finish(String),
    Io(String),
}

impl fmt::Display for FfmpegError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound(b) => write!(f, "{b} not found (set FFMPEG_PATH/FFPROBE_PATH or PATH)"),
            Self::Probe(e) => write!(f, "probe failed: {e}"),
            Self::Spawn(e) => write!(f, "spawn failed: {e}"),
            Self::Write(e) => write!(f, "frame write failed: {e}"),
            Self::Finish(e) => write!(f, "ffmpeg exited badly: {e}"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for FfmpegError {}

fn find_bin(env_key: &str, name: &str) -> Result<PathBuf, FfmpegError> {
    if let Some(p) = std::env::var_os(env_key) {
        let p = PathBuf::from(p);
        if p.is_file() {
            return Ok(p);
        }
    }
    let exe = if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_string()
    };
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|d| d.join(&exe))
                .find(|p| p.is_file())
        })
        .ok_or(FfmpegError::NotFound(if env_key == "FFMPEG_PATH" {
            "ffmpeg"
        } else {
            "ffprobe"
        }))
}

pub fn find_ffmpeg() -> Result<PathBuf, FfmpegError> {
    find_bin("FFMPEG_PATH", "ffmpeg")
}

pub fn find_ffprobe() -> Result<PathBuf, FfmpegError> {
    find_bin("FFPROBE_PATH", "ffprobe")
}

// ---------------------------------------------------------------------------
// probe
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MediaInfo {
    pub width: u32,
    pub height: u32,
    pub fps_num: u32,
    pub fps_den: u32,
    pub duration_secs: f64,
    pub has_video: bool,
    pub has_audio: bool,
    pub video_codec: String,
}

#[derive(Debug, Deserialize)]
struct ProbeOut {
    streams: Vec<ProbeStream>,
    format: ProbeFormat,
}

#[derive(Debug, Deserialize)]
struct ProbeStream {
    codec_type: Option<String>,
    codec_name: Option<String>,
    width: Option<u32>,
    height: Option<u32>,
    avg_frame_rate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ProbeFormat {
    duration: Option<String>,
}

fn parse_rate(s: &str) -> (u32, u32) {
    let mut it = s.split('/');
    let num: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    let den: u32 = it.next().and_then(|v| v.parse().ok()).unwrap_or(0);
    if num == 0 || den == 0 {
        (30, 1)
    } else {
        (num, den)
    }
}

pub fn probe(path: &Path) -> Result<MediaInfo, FfmpegError> {
    let ffprobe = find_ffprobe()?;
    let out = Command::new(ffprobe)
        .args([
            "-v",
            "error",
            "-print_format",
            "json",
            "-show_format",
            "-show_streams",
        ])
        .arg(path)
        .output()
        .map_err(|e| FfmpegError::Probe(e.to_string()))?;
    if !out.status.success() {
        return Err(FfmpegError::Probe(
            String::from_utf8_lossy(&out.stderr).into_owned(),
        ));
    }
    let parsed: ProbeOut =
        serde_json::from_slice(&out.stdout).map_err(|e| FfmpegError::Probe(e.to_string()))?;
    let video = parsed
        .streams
        .iter()
        .find(|s| s.codec_type.as_deref() == Some("video"));
    let audio = parsed
        .streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("audio"));
    let (fps_num, fps_den) = video
        .and_then(|v| v.avg_frame_rate.as_deref())
        .map(parse_rate)
        .unwrap_or((30, 1));
    Ok(MediaInfo {
        width: video.and_then(|v| v.width).unwrap_or(0),
        height: video.and_then(|v| v.height).unwrap_or(0),
        fps_num,
        fps_den,
        duration_secs: parsed
            .format
            .duration
            .and_then(|d| d.parse().ok())
            .unwrap_or(0.0),
        has_video: video.is_some(),
        has_audio: audio,
        video_codec: video.and_then(|v| v.codec_name.clone()).unwrap_or_default(),
    })
}

// ---------------------------------------------------------------------------
// mp4 書出し
// ---------------------------------------------------------------------------

/// 利用可能エンコーダから選ぶ（x264 優先、なければ mpeg4）。
pub fn pick_video_codec() -> Result<(&'static str, Vec<String>), FfmpegError> {
    let ffmpeg = find_ffmpeg()?;
    let out = Command::new(ffmpeg)
        .args(["-hide_banner", "-encoders"])
        .output()
        .map_err(|e| FfmpegError::Spawn(e.to_string()))?;
    let list = String::from_utf8_lossy(&out.stdout);
    if list.contains("libx264 ") || list.contains("libx264\n") || list.contains(" libx264") {
        Ok((
            "libx264",
            vec![
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-crf".to_string(),
                "20".to_string(),
                "-preset".to_string(),
                "veryfast".to_string(),
                "-movflags".to_string(),
                "+faststart".to_string(),
            ],
        ))
    } else {
        Ok((
            "mpeg4",
            vec![
                "-pix_fmt".to_string(),
                "yuv420p".to_string(),
                "-q:v".to_string(),
                "3".to_string(),
            ],
        ))
    }
}

pub struct Mp4Writer {
    child: Child,
    stdin: ChildStdin,
    width: u32,
    height: u32,
    frames: u64,
}

impl Mp4Writer {
    /// RGBA8 rawvideo を stdin に流して mp4 化する。
    pub fn new(out_path: &Path, width: u32, height: u32, fps: f64) -> Result<Self, FfmpegError> {
        let ffmpeg = find_ffmpeg()?;
        let (codec, extra) = pick_video_codec()?;
        let mut child = Command::new(ffmpeg)
            .args([
                "-y",
                "-f",
                "rawvideo",
                "-pix_fmt",
                "rgba",
                "-s",
                &format!("{width}x{height}"),
                "-framerate",
                &format!("{fps}"),
                "-i",
                "-",
                "-c:v",
                codec,
            ])
            .args(&extra)
            .arg(out_path)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| FfmpegError::Spawn(e.to_string()))?;
        let stdin = child
            .stdin
            .take()
            .ok_or(FfmpegError::Spawn("no stdin".to_string()))?;
        Ok(Self {
            child,
            stdin,
            width,
            height,
            frames: 0,
        })
    }

    pub fn write_frame(&mut self, rgba: &[u8]) -> Result<(), FfmpegError> {
        let expect = (self.width as usize) * (self.height as usize) * 4;
        if rgba.len() != expect {
            return Err(FfmpegError::Write(format!("bad frame size {}", rgba.len())));
        }
        self.stdin
            .write_all(rgba)
            .map_err(|e| FfmpegError::Write(e.to_string()))?;
        self.frames += 1;
        Ok(())
    }

    pub fn finish(mut self) -> Result<u64, FfmpegError> {
        drop(self.stdin);
        let status = self
            .child
            .wait()
            .map_err(|e| FfmpegError::Finish(e.to_string()))?;
        if !status.success() {
            return Err(FfmpegError::Finish(format!("status {status}")));
        }
        Ok(self.frames)
    }

    pub fn frames_written(&self) -> u64 {
        self.frames
    }
}

/// 先頭フレームを PNG サムネイル化する。
pub fn thumbnail(src: &Path, out_png: &Path, width: u32) -> Result<(), FfmpegError> {
    let ffmpeg = find_ffmpeg()?;
    let status = Command::new(ffmpeg)
        .args([
            "-y",
            "-v",
            "error",
            "-i",
            &src.to_string_lossy(),
            "-frames:v",
            "1",
            "-vf",
            &format!("scale={width}:-1"),
        ])
        .arg(out_png)
        .status()
        .map_err(|e| FfmpegError::Spawn(e.to_string()))?;
    if !status.success() {
        return Err(FfmpegError::Finish(format!("status {status}")));
    }
    Ok(())
}
