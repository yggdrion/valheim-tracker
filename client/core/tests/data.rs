mod support;

use serde_json::json;
use tracker_core::data::*;

#[test]
fn stat_names_has_205() {
    assert_eq!(STAT_NAMES.len(), 205);
}

#[test]
fn analysis_version_is_12_hex() {
    let v = analysis_version();
    assert_eq!(v.len(), 12);
    assert!(v.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f')), "{v}");
    assert_eq!(v, analysis_version());
}

#[test]
fn names_include_gamedata_tokens() {
    let sections = GAMEDATA["sections"].as_array().unwrap();
    assert!(!sections.is_empty());
    for s in sections {
        for e in s["entries"].as_array().unwrap() {
            let token = e["token"].as_str().unwrap();
            assert!(NAMES.contains_key(token), "{token}");
        }
    }
}

#[test]
fn fonts_embedded() {
    assert!(font("cinzel.woff2").is_some());
    assert!(font("x.woff2").is_none());
}

#[test]
fn embedded_files_are_present() {
    assert!(!LOCATION_NAMES.is_empty());
    assert!(STAT_MAP.is_object());
    assert!(ACHIEVEMENTS_JSON.starts_with(['{', '[']));
    assert!(DASHBOARD_HTML.contains("<html"));
    assert!(!WORLDGEN_JS.is_empty());
}

#[test]
fn close_compares_numbers_with_tolerance() {
    support::assert_json_close("$", &json!({"a": [1.0000000001, 2]}), &json!({"a": [1, 2.0]}));
    support::assert_json_close("$.meta.stats.X", &json!(10.1), &json!(10.0));
}

#[test]
#[should_panic(expected = "$.b[1]")]
fn close_reports_the_path_of_a_difference() {
    support::assert_json_close("$", &json!({"a": 1, "b": [1, 2]}), &json!({"a": 1, "b": [1, 3]}));
}

#[test]
#[should_panic(expected = "keys differ")]
fn close_requires_key_order() {
    support::assert_json_close("$", &json!({"a": 1, "b": 2}), &json!({"b": 2, "a": 1}));
}
