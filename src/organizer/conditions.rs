use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Condition {
    Comparison { field: String, op: CompareOp, value: String },
    And { conditions: Vec<Condition> },
    Or  { conditions: Vec<Condition> },
    Not { condition: Box<Condition> },
    Empty    { field: String },
    Nonempty { field: String },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompareOp { Eq, Ne, Contains, StartsWith, EndsWith }

pub fn eval_condition(cond: &Condition, tags: &HashMap<String, String>) -> bool {
    match cond {
        Condition::Comparison { field, op, value } => {
            let tag = tags.get(field).map(|s| s.to_lowercase()).unwrap_or_default();
            let val = value.to_lowercase();
            match op {
                CompareOp::Eq         => tag == val,
                CompareOp::Ne         => tag != val,
                CompareOp::Contains   => tag.contains(val.as_str()),
                CompareOp::StartsWith => tag.starts_with(val.as_str()),
                CompareOp::EndsWith   => tag.ends_with(val.as_str()),
            }
        }
        Condition::And { conditions } => conditions.iter().all(|c| eval_condition(c, tags)),
        Condition::Or  { conditions } => conditions.iter().any(|c| eval_condition(c, tags)),
        Condition::Not { condition }  => !eval_condition(condition, tags),
        Condition::Empty    { field } => tags.get(field).map(|v| v.is_empty()).unwrap_or(true),
        Condition::Nonempty { field } => tags.get(field).map(|v| !v.is_empty()).unwrap_or(false),
    }
}
