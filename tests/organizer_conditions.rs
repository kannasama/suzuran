use suzuran_server::organizer::{
    conditions::{eval_condition, Condition},
    rules::{apply_rules, match_rule},
};
use std::collections::HashMap;

fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

fn cond(json: serde_json::Value) -> Condition {
    serde_json::from_value(json).expect("invalid condition JSON")
}

// ── comparison ────────────────────────────────────────────────────────────────

#[test] fn eq_matches() {
    let c = cond(serde_json::json!({"type":"comparison","field":"genre","op":"eq","value":"classical"}));
    assert!(eval_condition(&c, &tags(&[("genre", "Classical")])));
}

#[test] fn eq_no_match() {
    let c = cond(serde_json::json!({"type":"comparison","field":"genre","op":"eq","value":"Classical"}));
    assert!(!eval_condition(&c, &tags(&[("genre", "Rock")])));
}

#[test] fn ne_matches() {
    let c = cond(serde_json::json!({"type":"comparison","field":"genre","op":"ne","value":"Rock"}));
    assert!(eval_condition(&c, &tags(&[("genre", "Jazz")])));
}

#[test] fn contains_matches() {
    let c = cond(serde_json::json!({"type":"comparison","field":"title","op":"contains","value":"numb"}));
    assert!(eval_condition(&c, &tags(&[("title", "Comfortably Numb")])));
}

#[test] fn starts_with_matches() {
    let c = cond(serde_json::json!({"type":"comparison","field":"albumartist","op":"starts_with","value":"pink"}));
    assert!(eval_condition(&c, &tags(&[("albumartist", "Pink Floyd")])));
}

#[test] fn ends_with_matches() {
    let c = cond(serde_json::json!({"type":"comparison","field":"albumartist","op":"ends_with","value":"floyd"}));
    assert!(eval_condition(&c, &tags(&[("albumartist", "Pink Floyd")])));
}

// ── logical ───────────────────────────────────────────────────────────────────

#[test] fn and_all_true() {
    let c = cond(serde_json::json!({
        "type": "and", "conditions": [
            {"type":"comparison","field":"genre","op":"eq","value":"Rock"},
            {"type":"nonempty","field":"albumartist"}
        ]
    }));
    assert!(eval_condition(&c, &tags(&[("genre","Rock"),("albumartist","AC/DC")])));
}

#[test] fn and_partial_false() {
    let c = cond(serde_json::json!({
        "type": "and", "conditions": [
            {"type":"comparison","field":"genre","op":"eq","value":"Rock"},
            {"type":"empty","field":"albumartist"}
        ]
    }));
    assert!(!eval_condition(&c, &tags(&[("genre","Rock"),("albumartist","AC/DC")])));
}

#[test] fn or_one_true() {
    let c = cond(serde_json::json!({
        "type": "or", "conditions": [
            {"type":"comparison","field":"genre","op":"eq","value":"Jazz"},
            {"type":"comparison","field":"genre","op":"eq","value":"Rock"}
        ]
    }));
    assert!(eval_condition(&c, &tags(&[("genre","Rock")])));
}

#[test] fn not_inverts() {
    let c = cond(serde_json::json!({
        "type": "not",
        "condition": {"type":"comparison","field":"genre","op":"eq","value":"Pop"}
    }));
    assert!(eval_condition(&c, &tags(&[("genre","Rock")])));
}

// ── presence ──────────────────────────────────────────────────────────────────

#[test] fn empty_when_absent() {
    let c = cond(serde_json::json!({"type":"empty","field":"label"}));
    assert!(eval_condition(&c, &tags(&[])));
}

#[test] fn empty_when_blank() {
    let c = cond(serde_json::json!({"type":"empty","field":"label"}));
    assert!(eval_condition(&c, &tags(&[("label","")])));
}

#[test] fn nonempty_when_present() {
    let c = cond(serde_json::json!({"type":"nonempty","field":"label"}));
    assert!(eval_condition(&c, &tags(&[("label","Harvest")])));
}

// ── rule matching ─────────────────────────────────────────────────────────────

#[test] fn null_condition_matches_all() {
    let result = match_rule(None, "{title}", &tags(&[("title","Song")]));
    assert_eq!(result, Some("Song".to_string()));
}

#[test] fn condition_no_match_returns_none() {
    let cond_json = serde_json::json!({"type":"comparison","field":"genre","op":"eq","value":"Classical"});
    let result = match_rule(Some(&cond_json), "{albumartist}/{title}", &tags(&[("genre","Rock")]));
    assert!(result.is_none());
}

#[test] fn apply_rules_first_match_wins() {
    let rules: Vec<(Option<serde_json::Value>, String)> = vec![
        (
            Some(serde_json::json!({"type":"comparison","field":"genre","op":"eq","value":"Classical"})),
            "Classical/{albumartist}/{title}".to_string(),
        ),
        (None, "{albumartist}/{title}".to_string()),
    ];
    let t = tags(&[("genre","Rock"),("albumartist","AC/DC"),("title","Highway to Hell")]);
    // First rule doesn't match (Rock != Classical), second matches (no condition)
    assert_eq!(
        apply_rules(&rules, &t),
        Some("AC/DC/Highway to Hell".to_string())
    );
}

#[test] fn apply_rules_classical_first() {
    let rules: Vec<(Option<serde_json::Value>, String)> = vec![
        (
            Some(serde_json::json!({"type":"comparison","field":"genre","op":"eq","value":"Classical"})),
            "Classical/{albumartist}/{title}".to_string(),
        ),
        (None, "{albumartist}/{title}".to_string()),
    ];
    let t = tags(&[("genre","Classical"),("albumartist","Bach"),("title","Cello Suite")]);
    assert_eq!(
        apply_rules(&rules, &t),
        Some("Classical/Bach/Cello Suite".to_string())
    );
}

#[test] fn apply_rules_no_match_returns_none() {
    let rules: Vec<(Option<serde_json::Value>, String)> = vec![(
        Some(serde_json::json!({"type":"comparison","field":"genre","op":"eq","value":"Jazz"})),
        "Jazz/{title}".to_string(),
    )];
    let t = tags(&[("genre","Rock"),("title","Riff")]);
    assert!(apply_rules(&rules, &t).is_none());
}
