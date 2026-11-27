mod support;
use indexmap::IndexMap;
use serde_json::Value;
use support::{assert_json_close, golden_dir, FchBuilder};
use tracker_core::analyze;
use tracker_core::data::GAMEDATA;
use tracker_core::fch;

fn parse(b: &FchBuilder) -> fch::Parsed {
    fch::parse(&b.build()).unwrap()
}

#[test]
fn section_counts_from_dicts() {
    let sec = GAMEDATA["sections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["source"] != "stats")
        .unwrap();
    let tok = sec["entries"][0]["token"].as_str().unwrap();
    let b = FchBuilder::new(47).dict(sec["set"].as_u64().unwrap() as usize, sec["source"].as_str().unwrap(), tok, 2.7);
    let secs = analyze::sections(&parse(&b));
    let s = secs.iter().find(|s| s.id == sec["id"].as_str().unwrap()).unwrap();
    assert_eq!(s.entries[0].count, 2);
    assert!(s.done >= 1);
    assert_eq!(s.total, s.entries.len());
}

fn kill_keys(b: &FchBuilder, field: &str) -> Vec<String> {
    let m = analyze::meta(&parse(b));
    m[field].as_object().unwrap().keys().cloned().collect()
}

#[test]
fn capitalize_fallback() {
    let b = FchBuilder::new(47).dict(0, "kills", "$enemy_greydwarf_elite", 1.0);
    assert_eq!(kill_keys(&b, "kills"), vec!["Greydwarf elite"]);
}

#[test]
fn meta_sorted_by_count_with_stable_ties() {
    let b = FchBuilder::new(47)
        .dict(0, "picked_up", "a", 1.0)
        .dict(0, "picked_up", "b", 5.0)
        .dict(0, "picked_up", "c", 5.0);
    assert_eq!(kill_keys(&b, "picked_up"), vec!["B", "C", "A"]);
}

#[test]
fn harvested_spaces() {
    let b = FchBuilder::new(47).dict(0, "pickables", "BlueberryBush", 3.0);
    assert_eq!(kill_keys(&b, "harvested"), vec!["Blueberry Bush"]);
}

#[test]
fn parity_analysis() {
    let Some(dir) = golden_dir() else { return };
    let manifest: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("manifest.json")).unwrap()).unwrap();
    let mut compared = 0;
    for c in manifest["characters"].as_array().unwrap() {
        let name = c["name"].as_str().unwrap();
        let want: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join(format!("{name}.analysis.json"))).unwrap()).unwrap();
        let raw = std::fs::read(c["path"].as_str().unwrap()).unwrap();
        let d = fch::parse(&raw).unwrap();
        let secs = analyze::sections(&d);
        let ach: IndexMap<String, [i64; 2]> = analyze::achievements(&d, &secs);
        assert_json_close(&format!("{name}.sections"), &serde_json::to_value(&secs).unwrap(), &want["sections"]);
        assert_json_close(&format!("{name}.achievements"), &serde_json::to_value(&ach).unwrap(), &want["achievements"]);
        assert_json_close(&format!("{name}.meta"), &analyze::meta(&d), &want["meta"]);
        compared += secs.len() + ach.len();
    }
    assert!(compared > 0);
}
