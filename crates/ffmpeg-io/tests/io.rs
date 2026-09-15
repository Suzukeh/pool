//! ffmpeg 実機テスト。未導入環境ではスキップ。

use pool_ffmpeg_io::{Mp4Writer, find_ffmpeg, pick_video_codec, probe, thumbnail};
use std::path::PathBuf;

fn available() -> bool {
    find_ffmpeg().is_ok()
}

fn tmp(name: &str) -> PathBuf {
    std::env::temp_dir().join(format!("pool-ffmpeg-{}-{name}", std::process::id()))
}

fn make_testsrc(out: &std::path::Path) {
    let ffmpeg = find_ffmpeg().unwrap();
    let st = std::process::Command::new(ffmpeg)
        .args([
            "-y",
            "-v",
            "error",
            "-f",
            "lavfi",
            "-i",
            "testsrc=s=128x72:d=1:r=30",
            "-pix_fmt",
            "yuv420p",
        ])
        .arg(out)
        .status()
        .unwrap();
    assert!(st.success());
}

#[test]
fn probe_testsrc() {
    if !available() {
        eprintln!("SKIP: ffmpeg not found");
        return;
    }
    let src = tmp("src.mp4");
    make_testsrc(&src);
    let info = probe(&src).unwrap();
    assert_eq!((info.width, info.height), (128, 72));
    assert!(info.has_video);
    assert!(info.duration_secs > 0.5);
    let _ = std::fs::remove_file(&src);
}

#[test]
fn mp4_roundtrip_synthetic_frames() {
    if !available() {
        eprintln!("SKIP: ffmpeg not found");
        return;
    }
    let out = tmp("out.mp4");
    let (w, h) = (64u32, 48u32);
    let mut writer = Mp4Writer::new(&out, w, h, 30.0).unwrap();
    for i in 0..10 {
        let mut frame = vec![0u8; (w * h * 4) as usize];
        for (p, px) in frame.chunks_exact_mut(4).enumerate() {
            px[0] = ((p + i as usize * 7) % 256) as u8;
            px[1] = 32;
            px[2] = 64;
            px[3] = 255;
        }
        writer.write_frame(&frame).unwrap();
    }
    assert_eq!(writer.finish().unwrap(), 10);
    let bytes = std::fs::read(&out).unwrap();
    assert!(bytes.len() > 1000, "mp4 too small: {}", bytes.len());
    let info = probe(&out).unwrap();
    assert_eq!((info.width, info.height), (w, h));
    let _ = std::fs::remove_file(&out);
}

#[test]
fn thumbnail_extracts_png() {
    if !available() {
        eprintln!("SKIP: ffmpeg not found");
        return;
    }
    let src = tmp("src2.mp4");
    let thumb = tmp("thumb.png");
    make_testsrc(&src);
    thumbnail(&src, &thumb, 64).unwrap();
    let bytes = std::fs::read(&thumb).unwrap();
    assert_eq!(&bytes[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
    let _ = std::fs::remove_file(&src);
    let _ = std::fs::remove_file(&thumb);
}

#[test]
fn codec_pick_reports_name() {
    if !available() {
        eprintln!("SKIP: ffmpeg not found");
        return;
    }
    let (name, _) = pick_video_codec().unwrap();
    assert!(!name.is_empty());
}
