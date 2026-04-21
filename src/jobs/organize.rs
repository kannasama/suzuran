use std::{collections::HashMap, path::Component, sync::Arc};
use serde_json::Value;
use tokio::fs;
use crate::{
    dal::Store,
    error::AppError,
    jobs::{JobHandler, OrganizePayload},
    organizer::rules::apply_rules,
};

pub struct OrganizeJobHandler;

#[async_trait::async_trait]
impl JobHandler for OrganizeJobHandler {
    async fn run(&self, db: Arc<dyn Store>, payload: Value) -> Result<Value, AppError> {
        let p: OrganizePayload = serde_json::from_value(payload)
            .map_err(|e| AppError::BadRequest(format!("invalid organize payload: {e}")))?;

        let track = db.get_track(p.track_id).await?
            .ok_or_else(|| AppError::NotFound(format!("track {} not found", p.track_id)))?;

        let library = db.get_library(track.library_id).await?
            .ok_or_else(|| AppError::NotFound(format!("library {} not found", track.library_id)))?;

        // Build tag map from the track's full tags JSON
        let tags: HashMap<String, String> = track.tags
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Load rules (global + library-scoped), priority-sorted ascending
        let rules_rows = db.list_organization_rules(Some(track.library_id)).await?;
        let rule_pairs: Vec<(Option<Value>, String)> = rules_rows
            .into_iter()
            .filter(|r| r.enabled)
            .map(|r| (r.conditions, r.path_template))
            .collect();

        let new_relative = apply_rules(&rule_pairs, &tags);

        // Guard against path traversal in rule output, regardless of dry_run mode
        if let Some(ref path) = new_relative {
            if std::path::Path::new(path).components().any(|c| {
                matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))
            }) {
                return Err(AppError::BadRequest(format!(
                    "organize rule produced an unsafe path: {path}"
                )));
            }
        }

        if p.dry_run {
            return Ok(serde_json::json!({ "dry_run": true, "proposed_path": new_relative }));
        }

        let new_relative = new_relative
            .ok_or_else(|| AppError::BadRequest(format!("no matching rule for track {}", p.track_id)))?;

        let old_abs = std::path::Path::new(&library.root_path).join(&track.relative_path);
        let new_abs = std::path::Path::new(&library.root_path).join(&new_relative);

        if let Some(parent) = new_abs.parent() {
            fs::create_dir_all(parent).await.map_err(|e| AppError::Internal(e.into()))?;
        }
        fs::rename(&old_abs, &new_abs).await.map_err(|e| AppError::Internal(e.into()))?;

        db.update_track_path(track.id, &new_relative, &track.file_hash).await?;

        Ok(serde_json::json!({
            "moved": true,
            "old_path": track.relative_path,
            "new_path": new_relative,
        }))
    }
}
