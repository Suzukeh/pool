use pool_aviutl_import::*;
use std::path::PathBuf;

fn fixture(name: &str) -> Vec<u8> {
    let p = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures")
        .join(name);
    std::fs::read(p).unwrap()
}

#[test]
fn utf8_multi_effect() {
    let s = parse_bytes(&fixture("sample.anm2"), "anm2", "sample");
    assert_eq!(s.kind, ScriptKind::Anim);
    assert_eq!(s.effects.len(), 2);
    let v = &s.effects[0];
    assert_eq!(v.name, "振動");
    assert_eq!(v.tracks.len(), 2);
    assert_eq!(v.tracks[0].name, "X移動");
    assert_eq!(
        (v.tracks[0].min, v.tracks[0].max, v.tracks[0].def),
        (-200.0, 200.0, 0.0)
    );
    assert_eq!(v.checks[0].name, "ランダム");
    assert!(!v.checks[0].def);
    assert_eq!(v.dialogs[0].text, "周期");
    assert_eq!(v.colors[0].name, "基準色");
    assert_eq!(v.script_lang.as_deref(), Some("luaJIT"));
    assert!(v.body.contains("obj.ox"));
    let r = &s.effects[1];
    assert_eq!(r.name, "回転");
    assert_eq!(r.files, vec!["mask.png".to_string()]);
}

#[test]
fn sjis_single_effect() {
    let s = parse_bytes(&fixture("sample_sjis.anm"), "anm", "sample_sjis");
    assert_eq!(s.kind, ScriptKind::Anim);
    assert_eq!(s.effects.len(), 1);
    let e = &s.effects[0];
    assert_eq!(e.name, "sample_sjis");
    assert_eq!(e.tracks[0].name, "大きさ");
    assert_eq!(
        (e.tracks[0].min, e.tracks[0].max, e.tracks[0].def),
        (0.0, 200.0, 100.0)
    );
    assert!(e.checks[0].def);
    assert!(e.body.contains("obj.zoom"));
}

#[test]
fn to_pool_filter_preserves_definition() {
    let s = parse_bytes(&fixture("sample.anm2"), "anm2", "sample");
    let o = s.effects[0].to_pool_filter("imp-1", 0, 90);
    assert_eq!((o.start_frame, o.end_frame), (0, 90));
    let get = |n: &str| {
        o.params
            .iter()
            .find(|p| p.name == n)
            .unwrap_or_else(|| panic!("missing {n}"))
            .value
            .clone()
    };
    assert!(matches!(
        get("track0"),
        pool_timeline_model::Value::Number(_)
    ));
    assert!(matches!(
        get("check0"),
        pool_timeline_model::Value::Boolean(false)
    ));
    assert!(matches!(
        get("script_body"),
        pool_timeline_model::Value::Text(_)
    ));
    // pool 検証も通ること
    let mut p = pool_timeline_model::Project::new("t");
    p.scenes[0].layers[0].objects.push(o);
    assert!(p.validate().is_ok());
}
