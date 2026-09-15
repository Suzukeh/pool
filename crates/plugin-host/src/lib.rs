//! 別プロセスプラグインのホスト。
//!
//! - 子プロセスを起動し、stdio NDJSON で対話（`pool-plugin-sdk` 参照）。
//! - フレームは RGBA8 ファイル受け渡し（M4。将来 mmap/DMA-BUF 化）。
//! - タイムアウト・異常終了はエラー化し、ホスト本体は生存する。

use pool_plugin_sdk::{FrameFile, InitMsg, Manifest, PluginKind, RenderMsg, Reply};
use std::fmt;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// フレームファイル置き場（存在しなければ作成）。
    pub workdir: PathBuf,
    pub init_timeout: Duration,
    pub render_timeout: Duration,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            workdir: std::env::temp_dir().join("pool-plugin-frames"),
            init_timeout: Duration::from_secs(10),
            render_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug)]
pub enum HostError {
    Spawn(String),
    Timeout(&'static str),
    Protocol(String),
    Plugin(String),
    Exited,
    Io(String),
}

impl fmt::Display for HostError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Spawn(e) => write!(f, "spawn failed: {e}"),
            Self::Timeout(op) => write!(f, "{op} timed out"),
            Self::Protocol(e) => write!(f, "protocol error: {e}"),
            Self::Plugin(e) => write!(f, "plugin error: {e}"),
            Self::Exited => write!(f, "plugin exited"),
            Self::Io(e) => write!(f, "io error: {e}"),
        }
    }
}

impl std::error::Error for HostError {}

#[derive(Debug)]
pub struct PluginHandle {
    child: Child,
    stdin: std::process::ChildStdin,
    rx: Receiver<Result<String, String>>,
    width: u32,
    height: u32,
    seq: u64,
    workdir: PathBuf,
    render_timeout: Duration,
}

impl PluginHandle {
    /// プラグインを起動し init  handshake する。
    pub fn spawn(
        manifest: &Manifest,
        plugin_dir: &Path,
        cfg: &HostConfig,
        width: u32,
        height: u32,
        params: &serde_json::Value,
    ) -> Result<Self, HostError> {
        manifest.validate().map_err(HostError::Protocol)?;
        let PluginKind::Command { program, args } = &manifest.kind;
        fs::create_dir_all(&cfg.workdir).map_err(|e| HostError::Io(e.to_string()))?;
        let mut child = Command::new(program)
            .args(args)
            .current_dir(plugin_dir)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null()) // M4: 捨てる（M5 でログ取り込み）
            .spawn()
            .map_err(|e| HostError::Spawn(format!("{program}: {e}")))?;

        let stdin = child
            .stdin
            .take()
            .ok_or(HostError::Spawn("no stdin".to_string()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or(HostError::Spawn("no stdout".to_string()))?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let reader = BufReader::new(stdout);
            for line in reader.lines() {
                let done = tx.send(line.map_err(|e| e.to_string())).is_err();
                if done {
                    break;
                }
            }
        });

        let mut handle = Self {
            child,
            stdin,
            rx,
            width,
            height,
            seq: 0,
            workdir: cfg.workdir.clone(),
            render_timeout: cfg.render_timeout,
        };
        handle.send(&InitMsg::new(width, height, params.clone()))?;
        match handle.recv(cfg.init_timeout)? {
            Reply::Ready => Ok(handle),
            Reply::Error { message } => Err(HostError::Plugin(message)),
            Reply::Done => Err(HostError::Protocol("unexpected done on init".to_string())),
        }
    }

    fn send(&mut self, v: &impl serde::Serialize) -> Result<(), HostError> {
        let mut s = serde_json::to_string(v).map_err(|e| HostError::Protocol(e.to_string()))?;
        s.push('\n');
        self.stdin
            .write_all(s.as_bytes())
            .map_err(|_| HostError::Exited)?;
        self.stdin.flush().map_err(|_| HostError::Exited)?;
        Ok(())
    }

    fn recv(&mut self, timeout: Duration) -> Result<Reply, HostError> {
        let line = self
            .rx
            .recv_timeout(timeout)
            .map_err(|_| HostError::Timeout("plugin reply"))?
            .map_err(|_| HostError::Exited)?;
        serde_json::from_str(&line).map_err(|e| HostError::Protocol(e.to_string()))
    }

    fn frame_paths(&mut self) -> (PathBuf, PathBuf) {
        self.seq += 1;
        let pid = std::process::id();
        (
            self.workdir
                .join(format!("pool-in-{pid}-{}.rgba", self.seq)),
            self.workdir
                .join(format!("pool-out-{pid}-{}.rgba", self.seq)),
        )
    }

    /// 1 フレーム描画。入力 RGBA8（W*H*4）を渡し、出力 RGBA8 を返す。
    pub fn render(
        &mut self,
        frame: i64,
        input_rgba: &[u8],
        params: &serde_json::Value,
    ) -> Result<Vec<u8>, HostError> {
        let expect = (self.width as usize) * (self.height as usize) * 4;
        if input_rgba.len() != expect {
            return Err(HostError::Protocol(format!(
                "bad input size: {} != {expect}",
                input_rgba.len()
            )));
        }
        if !self.alive() {
            return Err(HostError::Exited);
        }
        let (in_path, out_path) = self.frame_paths();
        fs::write(&in_path, input_rgba).map_err(|e| HostError::Io(e.to_string()))?;
        let msg = RenderMsg {
            v: pool_plugin_sdk::PROTOCOL_VERSION,
            kind: "render".to_string(),
            frame,
            input: FrameFile {
                path: in_path.to_string_lossy().into_owned(),
                size: expect as u64,
            },
            output: FrameFile {
                path: out_path.to_string_lossy().into_owned(),
                size: expect as u64,
            },
            params: params.clone(),
        };
        let result = (|| {
            self.send(&msg)?;
            match self.recv(self.render_timeout)? {
                Reply::Done => {
                    let bytes = fs::read(&out_path).map_err(|e| HostError::Io(e.to_string()))?;
                    if bytes.len() != expect {
                        return Err(HostError::Protocol(format!(
                            "bad output size: {} != {expect}",
                            bytes.len()
                        )));
                    }
                    Ok(bytes)
                }
                Reply::Error { message } => Err(HostError::Plugin(message)),
                Reply::Ready => Err(HostError::Protocol("unexpected ready".to_string())),
            }
        })();
        let _ = fs::remove_file(&in_path);
        let _ = fs::remove_file(&out_path);
        result
    }

    pub fn alive(&mut self) -> bool {
        matches!(self.child.try_wait(), Ok(None))
    }

    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

impl Drop for PluginHandle {
    fn drop(&mut self) {
        self.kill();
    }
}

/// プラグインディレクトリを導入（検証＋コピー）。zip 展開は M5。
pub fn install_from_dir(src: &Path, plugins_dir: &Path) -> Result<Manifest, HostError> {
    let manifest_path = src.join("manifest.json");
    let json = fs::read_to_string(&manifest_path).map_err(|e| HostError::Io(e.to_string()))?;
    let manifest = Manifest::load(&json).map_err(HostError::Protocol)?;
    let safe: String = manifest
        .name
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
        .collect();
    if safe.is_empty() || safe != manifest.name {
        return Err(HostError::Protocol("unsafe plugin name".to_string()));
    }
    let dst = plugins_dir.join(&safe);
    copy_dir(src, &dst)?;
    Ok(manifest)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), HostError> {
    fs::create_dir_all(dst).map_err(|e| HostError::Io(e.to_string()))?;
    for entry in fs::read_dir(src).map_err(|e| HostError::Io(e.to_string()))? {
        let entry = entry.map_err(|e| HostError::Io(e.to_string()))?;
        let ty = entry
            .file_type()
            .map_err(|e| HostError::Io(e.to_string()))?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else if ty.is_file() {
            fs::copy(entry.path(), to).map_err(|e| HostError::Io(e.to_string()))?;
        }
    }
    Ok(())
}
