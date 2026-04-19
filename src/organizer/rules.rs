use crate::organizer::{conditions::{eval_condition, Condition}, template::render_template};
use std::collections::HashMap;

/// Returns rendered path if `conditions` match (or conditions is None), else None.
pub fn match_rule(
    conditions: Option<&serde_json::Value>,
    path_template: &str,
    tags: &HashMap<String, String>,
) -> Option<String> {
    let matches = match conditions {
        None => true,
        Some(v) => serde_json::from_value::<Condition>(v.clone())
            .map(|c| eval_condition(&c, tags))
            .unwrap_or(false),
    };
    if matches { Some(render_template(path_template, tags)) } else { None }
}

/// Evaluate a priority-ordered rule list. Returns the first matching rendered path.
/// `rules` must be sorted by priority ascending before calling.
pub fn apply_rules(
    rules: &[(Option<serde_json::Value>, String)],
    tags: &HashMap<String, String>,
) -> Option<String> {
    rules.iter().find_map(|(cond, tmpl)| match_rule(cond.as_ref(), tmpl, tags))
}
