//! 別プロセス E2E：Python サンプルで反転描画＋異常系。

use pool_plugin_host::{HostConfig, PluginHandle, install_from_dir};
use pool_plugin_sdk::Manifest;
use std::path::PathBuf;
use std::process::Command;

fn example_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../plugins/examples/hello-python")
}

fn python3_available() -> bool {
    Command::new("python3")
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn test_config() -> HostConfig {
    HostConfig {
        workdir: std::env::temp_dir().join(format!("pool-e2e-{}", std::process::id())),
        ..Default::default()
    }
}

#[test]
fn invert_roundtrip() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }
    let dir = example_dir();
    let json = std::fs::read_to_string(dir.join("manifest.json")).unwrap();
    let manifest = Manifest::load(&json).unwrap();
    let cfg = test_config();
    let mut h = PluginHandle::spawn(&manifest, &dir, &cfg, 4, 2, &serde_json::json!({})).unwrap();

    // 4x2 フレーム：連番バイト
    let input: Vec<u8> = (0..32u8).collect();
    let out = h
        .render(0, &input, &serde_json::json!({ "amount": 1.0 }))
        .unwrap();
    assert_eq!(out.len(), 32);
    for (a, b) in input.iter().zip(out.iter()) {
        assert_eq!(*b, 255 - *a);
    }

    // amount=0 は素通し
    let out = h
        .render(1, &input, &serde_json::json!({ "amount": 0.0 }))
        .unwrap();
    assert_eq!(out, input);
}

#[test]
fn dead_plugin_fails_to_spawn() {
    let manifest = Manifest {
        name: "dead".to_string(),
        version: "0.1.0".to_string(),
        description: String::new(),
        kind: pool_plugin_sdk::PluginKind::Command {
            program: "false".to_string(),
            args: vec![],
        },
    };
    let cfg = test_config();
    let err = PluginHandle::spawn(
        &manifest,
        std::env::temp_dir().as_path(),
        &cfg,
        4,
        2,
        &serde_json::json!({}),
    )
    .unwrap_err();
    // 即死：EOF による Exited/Timeout/Protocol のいずれか（ホストは生存）
    let msg = err.to_string();
    assert!(
        msg.contains("exited") || msg.contains("timed out") || msg.contains("protocol"),
        "unexpected: {msg}"
    );
}

#[test]
fn missing_program_fails_to_spawn() {
    let manifest = Manifest {
        name: "missing".to_string(),
        version: "0.1.0".to_string(),
        description: String::new(),
        kind: pool_plugin_sdk::PluginKind::Command {
            program: "pool-no-such-program-xyz".to_string(),
            args: vec![],
        },
    };
    let cfg = test_config();
    let err = PluginHandle::spawn(
        &manifest,
        std::env::temp_dir().as_path(),
        &cfg,
        4,
        2,
        &serde_json::json!({}),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("spawn failed"),
        "unexpected: {err}"
    );
}

#[test]
fn install_from_dir_copies_and_validates() {
    if !python3_available() {
        eprintln!("SKIP: python3 not found");
        return;
    }
    let dst_root = std::env::temp_dir().join(format!("pool-plugins-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dst_root);
    let manifest = install_from_dir(&example_dir(), &dst_root).unwrap();
    assert_eq!(manifest.name, "hello-python");
    assert!(dst_root.join("hello-python").join("plugin.py").exists());
    let _ = std::fs::remove_dir_all(&dst_root);
}
