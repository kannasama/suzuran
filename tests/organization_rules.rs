use std::sync::Arc;
use suzuran_server::dal::{sqlite::SqliteStore, Store};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

#[tokio::test]
async fn org_rule_crud() {
    let db = make_db().await;

    // Create a library to scope one of our rules to
    let lib = db.create_library("FLAC", "/music/flac", "flac").await.unwrap();

    // Create a global rule (library_id = None)
    let global = db
        .create_organization_rule(
            "Global Default",
            None,
            0,
            None,
            "{albumartist}/{date} - {album}/{tracknumber:02} - {title}",
            true,
        )
        .await
        .unwrap();
    assert_eq!(global.name, "Global Default");
    assert!(global.library_id.is_none());
    assert!(global.conditions.is_none());

    // Create a library-scoped rule with conditions
    let cond = serde_json::json!({
        "type": "comparison",
        "field": "genre",
        "op": "eq",
        "value": "Classical"
    });
    let scoped = db
        .create_organization_rule(
            "Classical",
            Some(lib.id),
            10,
            Some(cond.clone()),
            "Classical/{albumartist}/{date} - {album}/{tracknumber:02} - {title}",
            true,
        )
        .await
        .unwrap();
    assert_eq!(scoped.library_id, Some(lib.id));
    assert_eq!(scoped.conditions, Some(cond));

    // list_organization_rules(None) returns both
    let all = db.list_organization_rules(None).await.unwrap();
    assert_eq!(all.len(), 2);
    // sorted by priority ascending
    assert_eq!(all[0].priority, 0);
    assert_eq!(all[1].priority, 10);

    // list_organization_rules(Some(lib.id)) returns global + scoped for this library
    let for_lib = db.list_organization_rules(Some(lib.id)).await.unwrap();
    assert_eq!(for_lib.len(), 2);

    // get_organization_rule
    let fetched = db.get_organization_rule(global.id).await.unwrap();
    assert!(fetched.is_some());
    assert_eq!(fetched.unwrap().name, "Global Default");

    // update_organization_rule — rename + disable
    let updated = db
        .update_organization_rule(global.id, "Renamed Global", 5, None, "{title}", false)
        .await
        .unwrap();
    assert!(updated.is_some());
    let updated = updated.unwrap();
    assert_eq!(updated.name, "Renamed Global");
    assert_eq!(updated.priority, 5);
    assert!(!updated.enabled);

    // delete_organization_rule
    db.delete_organization_rule(scoped.id).await.unwrap();
    let after_delete = db.list_organization_rules(None).await.unwrap();
    assert_eq!(after_delete.len(), 1);
}

#[tokio::test]
async fn get_nonexistent_rule_returns_none() {
    let db = make_db().await;
    let result = db.get_organization_rule(9999).await.unwrap();
    assert!(result.is_none());
}
