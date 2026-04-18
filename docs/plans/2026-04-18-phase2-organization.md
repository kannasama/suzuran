# Phase 2 — Organization Engine Implementation Plan

> **For Claude:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task.

**Goal:** Implement the organization rule engine — condition-based rules matched against track tags, path template rendering, file-move jobs, CRUD API, and management UI.

**Architecture:** A new `organization_rules` DB table holds rules evaluated in priority order.
The path template engine (`src/organizer/template.rs`) tokenizes `{field}`, `{field:02}`,
`{field|fallback}`, and `{discfolder}` patterns. The condition evaluator
(`src/organizer/conditions.rs`) deserializes a JSON expression tree and tests it against a
track's tag map. An `organize` job type plugs into the existing `JobHandler` / scheduler
infrastructure. All new API routes follow the exact pattern of `src/api/libraries.rs`.

**Tech Stack:** All deps already in Cargo.toml (sqlx, serde_json, tokio, walkdir). No new
Rust or npm packages required.

---

## Branch setup

```bash
git checkout main
git checkout -b 0.2
git checkout -b 0.2.1 0.2
```

All Phase 2 subphase branches cut from `0.2`, not `main`.

---

## Subphase 2.1 — Organization Rules: Migration & DAL

**Files:**
- Create: `migrations/postgres/0008_organization_rules.sql`
- Create: `migrations/sqlite/0008_organization_rules.sql`
- Modify: `src/models/mod.rs` — add `OrganizationRule`
- Modify: `src/dal/mod.rs` — add 5 Store trait methods
- Modify: `src/dal/postgres.rs` — implement those methods
- Modify: `src/dal/sqlite.rs` — implement those methods
- Create: `tests/organization_rules.rs`
- Modify: `tasks/codebase-filemap.md`

### Task 1: Create Postgres migration

Create `migrations/postgres/0008_organization_rules.sql`:

```sql
CREATE TABLE organization_rules (
    id            BIGSERIAL PRIMARY KEY,
    name          TEXT NOT NULL,
    library_id    BIGINT REFERENCES libraries(id) ON DELETE CASCADE,
    priority      INTEGER NOT NULL DEFAULT 0,
    conditions    JSONB,
    path_template TEXT NOT NULL,
    enabled       BOOLEAN NOT NULL DEFAULT TRUE,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_org_rules_library_id ON organization_rules(library_id);
CREATE INDEX idx_org_rules_priority   ON organization_rules(priority);
```

### Task 2: Create SQLite migration

Create `migrations/sqlite/0008_organization_rules.sql`:

```sql
CREATE TABLE organization_rules (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    name          TEXT NOT NULL,
    library_id    INTEGER REFERENCES libraries(id) ON DELETE CASCADE,
    priority      INTEGER NOT NULL DEFAULT 0,
    conditions    TEXT,
    path_template TEXT NOT NULL,
    enabled       INTEGER NOT NULL DEFAULT 1,
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX idx_org_rules_library_id ON organization_rules(library_id);
CREATE INDEX idx_org_rules_priority   ON organization_rules(priority);
```

### Task 3: Write the failing test

Create `tests/organization_rules.rs`. Use the same `SqliteStore::new("sqlite::memory:")`
pattern as `tests/scanner.rs` — no HTTP server needed here:

```rust
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
    let lib = db.create_library("FLAC", "/music/flac", "flac", None).await.unwrap();

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
```

Run: `docker buildx build --progress=plain -t suzuran:dev .`
Expected: compile error — `create_organization_rule` not on Store trait yet.

### Task 4: Add `OrganizationRule` to models

In `src/models/mod.rs`, append after the `Theme` struct:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct OrganizationRule {
    pub id: i64,
    pub name: String,
    pub library_id: Option<i64>,
    pub priority: i32,
    pub conditions: Option<serde_json::Value>,
    pub path_template: String,
    pub enabled: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
```

### Task 5: Add Store trait methods

In `src/dal/mod.rs`:
1. Add `OrganizationRule` to the `use crate::models::{...}` import.
2. Append a new section to the `Store` trait:

```rust
// ── organization rules ────────────────────────────────────
/// Returns all rules when library_id is None; when Some, returns global rules
/// (library_id IS NULL) plus rules scoped to that library, ordered by priority asc.
async fn list_organization_rules(&self, library_id: Option<i64>) -> Result<Vec<OrganizationRule>, AppError>;
async fn get_organization_rule(&self, id: i64) -> Result<Option<OrganizationRule>, AppError>;
async fn create_organization_rule(
    &self,
    name: &str,
    library_id: Option<i64>,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: &str,
    enabled: bool,
) -> Result<OrganizationRule, AppError>;
async fn update_organization_rule(
    &self,
    id: i64,
    name: &str,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: &str,
    enabled: bool,
) -> Result<Option<OrganizationRule>, AppError>;
async fn delete_organization_rule(&self, id: i64) -> Result<(), AppError>;
```

### Task 6: Implement PgStore methods

In `src/dal/postgres.rs`, add after the `delete_library` impl (follow the `query_as::<_, T>(sql).bind(...)` pattern used throughout):

```rust
async fn list_organization_rules(&self, library_id: Option<i64>) -> Result<Vec<OrganizationRule>, AppError> {
    let rows = if let Some(lid) = library_id {
        sqlx::query_as::<_, OrganizationRule>(
            "SELECT * FROM organization_rules
             WHERE library_id IS NULL OR library_id = $1
             ORDER BY priority ASC",
        )
        .bind(lid)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, OrganizationRule>(
            "SELECT * FROM organization_rules ORDER BY priority ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?
    };
    Ok(rows)
}

async fn get_organization_rule(&self, id: i64) -> Result<Option<OrganizationRule>, AppError> {
    sqlx::query_as::<_, OrganizationRule>("SELECT * FROM organization_rules WHERE id = $1")
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(AppError::Database)
}

async fn create_organization_rule(
    &self,
    name: &str,
    library_id: Option<i64>,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: &str,
    enabled: bool,
) -> Result<OrganizationRule, AppError> {
    sqlx::query_as::<_, OrganizationRule>(
        "INSERT INTO organization_rules (name, library_id, priority, conditions, path_template, enabled)
         VALUES ($1, $2, $3, $4, $5, $6) RETURNING *",
    )
    .bind(name)
    .bind(library_id)
    .bind(priority)
    .bind(conditions)
    .bind(path_template)
    .bind(enabled)
    .fetch_one(&self.pool)
    .await
    .map_err(AppError::Database)
}

async fn update_organization_rule(
    &self,
    id: i64,
    name: &str,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: &str,
    enabled: bool,
) -> Result<Option<OrganizationRule>, AppError> {
    sqlx::query_as::<_, OrganizationRule>(
        "UPDATE organization_rules
         SET name=$1, priority=$2, conditions=$3, path_template=$4, enabled=$5
         WHERE id=$6 RETURNING *",
    )
    .bind(name)
    .bind(priority)
    .bind(conditions)
    .bind(path_template)
    .bind(enabled)
    .bind(id)
    .fetch_optional(&self.pool)
    .await
    .map_err(AppError::Database)
}

async fn delete_organization_rule(&self, id: i64) -> Result<(), AppError> {
    sqlx::query("DELETE FROM organization_rules WHERE id = $1")
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
}
```

### Task 7: Implement SqliteStore methods

In `src/dal/sqlite.rs`, add the same 5 methods. The SQL is identical except use `?` placeholders
instead of `$1`. `serde_json::Value` binds correctly to SQLite TEXT via sqlx's `json` feature.

```rust
async fn list_organization_rules(&self, library_id: Option<i64>) -> Result<Vec<OrganizationRule>, AppError> {
    let rows = if let Some(lid) = library_id {
        sqlx::query_as::<_, OrganizationRule>(
            "SELECT * FROM organization_rules
             WHERE library_id IS NULL OR library_id = ?
             ORDER BY priority ASC",
        )
        .bind(lid)
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?
    } else {
        sqlx::query_as::<_, OrganizationRule>(
            "SELECT * FROM organization_rules ORDER BY priority ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(AppError::Database)?
    };
    Ok(rows)
}

// get / create / update / delete: identical pattern with ? placeholders
```

### Task 8: Run tests

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected output includes:
```
test org_rule_crud ... ok
test get_nonexistent_rule_returns_none ... ok
```

If the `OrganizationRule` `FromRow` decode fails for `conditions` on SQLite, check that the
`json` sqlx feature is enabled (it is — `features = [..., "json"]` in Cargo.toml). No manual
serialization needed.

### Task 9: Update codebase filemap

In `tasks/codebase-filemap.md` add:
- `migrations/postgres/0008_organization_rules.sql` and SQLite counterpart
- `tests/organization_rules.rs`

### Task 10: Commit

```bash
git add migrations/ src/models/mod.rs src/dal/ tests/organization_rules.rs tasks/codebase-filemap.md
git commit -m "feat(2.1): organization_rules migration, DAL methods, and store tests"
```

---

## Subphase 2.2 — Path Template Engine

**Files:**
- Create: `src/organizer/mod.rs`
- Create: `src/organizer/template.rs`
- Modify: `src/lib.rs` — add `pub mod organizer;`
- Create: `tests/organizer_template.rs`
- Modify: `tasks/codebase-filemap.md`

**Token spec:**

| Token | Meaning |
|-------|---------|
| `{field}` | Raw value from tags map; empty string if missing |
| `{field:02}` | Parse value as u32, zero-pad to width 2 |
| `{field\|fallback}` | Use `fallback` literal when field absent or blank |
| `{discfolder}` | `"Disc N/"` when `totaldiscs > 1`, otherwise `""` |

### Task 1: Write the failing tests

Create `tests/organizer_template.rs`:

```rust
use suzuran_server::organizer::template::render_template;
use std::collections::HashMap;

fn tags(pairs: &[(&str, &str)]) -> HashMap<String, String> {
    pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
}

#[test]
fn simple_field_substitution() {
    let t = tags(&[("albumartist", "Air"), ("title", "La Femme d'Argent")]);
    assert_eq!(render_template("{albumartist}/{title}", &t), "Air/La Femme d'Argent");
}

#[test]
fn missing_field_renders_empty() {
    assert_eq!(render_template("{title}", &tags(&[])), "");
}

#[test]
fn padded_track_number() {
    let t = tags(&[("tracknumber", "6")]);
    assert_eq!(render_template("{tracknumber:02}", &t), "06");
}

#[test]
fn padded_already_wide_number() {
    let t = tags(&[("tracknumber", "12")]);
    assert_eq!(render_template("{tracknumber:02}", &t), "12");
}

#[test]
fn fallback_used_when_field_absent() {
    assert_eq!(render_template("{albumartist|Various Artists}", &tags(&[])), "Various Artists");
}

#[test]
fn fallback_used_when_field_blank() {
    let t = tags(&[("albumartist", "")]);
    assert_eq!(render_template("{albumartist|Various Artists}", &t), "Various Artists");
}

#[test]
fn fallback_not_used_when_field_present() {
    let t = tags(&[("albumartist", "Air")]);
    assert_eq!(render_template("{albumartist|Various Artists}", &t), "Air");
}

#[test]
fn discfolder_multi_disc() {
    let t = tags(&[("totaldiscs", "2"), ("discnumber", "2")]);
    assert_eq!(render_template("{discfolder}", &t), "Disc 2/");
}

#[test]
fn discfolder_single_disc_suppressed() {
    let t = tags(&[("totaldiscs", "1"), ("discnumber", "1")]);
    assert_eq!(render_template("{discfolder}", &t), "");
}

#[test]
fn discfolder_absent_tags_suppressed() {
    assert_eq!(render_template("{discfolder}", &tags(&[])), "");
}

#[test]
fn full_template_multi_disc() {
    let t = tags(&[
        ("albumartist", "Pink Floyd"), ("date", "1979"), ("album", "The Wall"),
        ("totaldiscs", "2"), ("discnumber", "2"), ("tracknumber", "6"),
        ("title", "Comfortably Numb"),
    ]);
    assert_eq!(
        render_template(
            "{albumartist}/{date} - {album}/{discfolder}{tracknumber:02} - {title}",
            &t
        ),
        "Pink Floyd/1979 - The Wall/Disc 2/06 - Comfortably Numb"
    );
}

#[test]
fn full_template_single_disc() {
    let t = tags(&[
        ("albumartist", "Air"), ("date", "1998"), ("album", "Moon Safari"),
        ("totaldiscs", "1"), ("discnumber", "1"), ("tracknumber", "1"),
        ("title", "La Femme d'Argent"),
    ]);
    assert_eq!(
        render_template(
            "{albumartist}/{date} - {album}/{discfolder}{tracknumber:02} - {title}",
            &t
        ),
        "Air/1998 - Moon Safari/01 - La Femme d'Argent"
    );
}
```

Run: `docker buildx build --progress=plain -t suzuran:dev .`
Expected: compile error — `organizer` module doesn't exist.

### Task 2: Create module skeleton

Create `src/organizer/mod.rs`:
```rust
pub mod template;
```

Add to `src/lib.rs` (after existing `pub mod` lines):
```rust
pub mod organizer;
```

### Task 3: Implement `render_template`

Create `src/organizer/template.rs`:

```rust
use std::collections::HashMap;

/// Render a path template against a track's tag map.
///
/// Supported tokens:
///   {field}          — raw value, empty string if absent
///   {field:02}       — zero-padded integer (width = the number after colon)
///   {field|fallback} — use fallback literal if field absent or blank
///   {discfolder}     — "Disc N/" when totaldiscs > 1, else ""
pub fn render_template(template: &str, tags: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(template.len() * 2);
    let mut chars = template.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '{' {
            let mut token = String::new();
            for inner in chars.by_ref() {
                if inner == '}' { break; }
                token.push(inner);
            }
            out.push_str(&resolve_token(&token, tags));
        } else {
            out.push(ch);
        }
    }
    out
}

fn resolve_token(token: &str, tags: &HashMap<String, String>) -> String {
    // synthetic: {discfolder}
    if token == "discfolder" {
        let total: u32 = tags.get("totaldiscs").and_then(|s| s.parse().ok()).unwrap_or(0);
        return if total > 1 {
            let disc: u32 = tags.get("discnumber").and_then(|s| s.parse().ok()).unwrap_or(1);
            format!("Disc {}/", disc)
        } else {
            String::new()
        };
    }

    // {field|fallback}
    if let Some((field, fallback)) = token.split_once('|') {
        let val = tags.get(field).map(|s| s.trim()).unwrap_or("");
        return if val.is_empty() { fallback.to_string() } else { val.to_string() };
    }

    // {field:width} — zero-padded integer
    if let Some((field, fmt)) = token.split_once(':') {
        if let Ok(width) = fmt.parse::<usize>() {
            let raw = tags.get(field).map(|s| s.as_str()).unwrap_or("");
            let n: u32 = raw.parse().unwrap_or(0);
            return format!("{:0>width$}", n, width = width);
        }
    }

    // {field} — raw value
    tags.get(token).cloned().unwrap_or_default()
}
```

### Task 4: Run tests, verify all pass

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: all `organizer_template` tests green.

### Task 5: Update filemap + commit

```bash
git add src/organizer/ src/lib.rs tests/organizer_template.rs tasks/codebase-filemap.md
git commit -m "feat(2.2): path template engine ({field}, :pad, |fallback, {discfolder})"
```

---

## Subphase 2.3 — Condition Evaluator & Rule Matcher

**Files:**
- Create: `src/organizer/conditions.rs`
- Create: `src/organizer/rules.rs`
- Modify: `src/organizer/mod.rs` — add both modules
- Create: `tests/organizer_conditions.rs`
- Modify: `tasks/codebase-filemap.md`

**Condition JSON schema:**

```json
// Field comparison (op: eq | ne | contains | starts_with | ends_with)
{ "type": "comparison", "field": "genre", "op": "eq", "value": "Classical" }

// Logical composites
{ "type": "and", "conditions": [...] }
{ "type": "or",  "conditions": [...] }
{ "type": "not", "condition": {...} }

// Field presence
{ "type": "empty",    "field": "albumartist" }
{ "type": "nonempty", "field": "albumartist" }

// null in DB = match all tracks
```

All comparisons are case-insensitive.

### Task 1: Write failing tests

Create `tests/organizer_conditions.rs`:

```rust
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
    // First rule doesn't match (Rock ≠ Classical), second matches (no condition)
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
```

Run: `docker buildx build --progress=plain -t suzuran:dev .`
Expected: compile error — `organizer::conditions` doesn't exist.

### Task 2: Implement `Condition` + `eval_condition`

Create `src/organizer/conditions.rs`:

```rust
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
                CompareOp::Eq          => tag == val,
                CompareOp::Ne          => tag != val,
                CompareOp::Contains    => tag.contains(val.as_str()),
                CompareOp::StartsWith  => tag.starts_with(val.as_str()),
                CompareOp::EndsWith    => tag.ends_with(val.as_str()),
            }
        }
        Condition::And { conditions } => conditions.iter().all(|c| eval_condition(c, tags)),
        Condition::Or  { conditions } => conditions.iter().any(|c| eval_condition(c, tags)),
        Condition::Not { condition }  => !eval_condition(condition, tags),
        Condition::Empty    { field } => tags.get(field).map(|v| v.is_empty()).unwrap_or(true),
        Condition::Nonempty { field } => tags.get(field).map(|v| !v.is_empty()).unwrap_or(false),
    }
}
```

### Task 3: Implement `match_rule` + `apply_rules`

Create `src/organizer/rules.rs`:

```rust
use crate::organizer::{conditions::{eval_condition, Condition}, template::render_template};
use std::collections::HashMap;

/// Returns rendered path if `conditions` match (or is None), else None.
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
```

### Task 4: Export from organizer

Update `src/organizer/mod.rs`:
```rust
pub mod conditions;
pub mod rules;
pub mod template;
```

### Task 5: Run tests, verify all pass

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: all `organizer_conditions` tests green.

### Task 6: Update filemap + commit

```bash
git add src/organizer/ tests/organizer_conditions.rs tasks/codebase-filemap.md
git commit -m "feat(2.3): condition evaluator and rule matcher"
```

---

## Subphase 2.4 — Organize Job Handler

**Files:**
- Modify: `src/dal/mod.rs` — add `update_track_path` to Store trait
- Modify: `src/dal/postgres.rs` — implement it
- Modify: `src/dal/sqlite.rs` — implement it
- Create: `src/jobs/organize.rs`
- Modify: `src/jobs/mod.rs` — add `OrganizePayload`, re-export
- Modify: `src/scheduler/mod.rs` — register `"organize"` job type
- Create: `tests/organize_job.rs`
- Modify: `tasks/codebase-filemap.md`

**OrganizePayload JSON:**
```json
{ "track_id": 42, "dry_run": false }
```

**Handler logic:**
1. Deserialize payload → `OrganizePayload`
2. `store.get_track(track_id)` → track (error if not found)
3. `store.get_library(track.library_id)` → library (error if not found)
4. `store.list_organization_rules(Some(track.library_id))` → rules (priority-sorted)
5. Build `tags: HashMap<String, String>` from `track.tags.as_object()`
6. `apply_rules(&rule_pairs, &tags)` → `new_relative_path` (error if no match and not dry_run)
7. If `dry_run`: return `json!({"dry_run":true,"proposed_path":new_path})`
8. Rename `{library.root_path}/{track.relative_path}` → `{library.root_path}/{new_path}`
   (create parent dirs with `tokio::fs::create_dir_all`)
9. `store.update_track_path(track.id, &new_relative_path)` → update DB
10. Return `json!({"moved":true,"old_path":old,"new_path":new})`

### Task 1: Add `update_track_path` to Store trait

In `src/dal/mod.rs`, add to the tracks section:

```rust
async fn update_track_path(&self, id: i64, relative_path: &str) -> Result<(), AppError>;
```

In `src/dal/postgres.rs`:
```rust
async fn update_track_path(&self, id: i64, relative_path: &str) -> Result<(), AppError> {
    sqlx::query("UPDATE tracks SET relative_path = $1 WHERE id = $2")
        .bind(relative_path)
        .bind(id)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(AppError::Database)
}
```

In `src/dal/sqlite.rs` — identical with `?` placeholders.

### Task 2: Write failing test

Create `tests/organize_job.rs`:

```rust
use std::sync::Arc;
use tokio::fs;
use suzuran_server::{
    dal::{sqlite::SqliteStore, Store},
    jobs::{organize::OrganizeJobHandler, JobHandler, OrganizePayload},
};

async fn make_db() -> Arc<dyn Store> {
    let store = SqliteStore::new("sqlite::memory:").await.unwrap();
    store.migrate().await.unwrap();
    Arc::new(store)
}

#[tokio::test]
async fn organize_moves_file_and_updates_path() {
    let db = make_db().await;

    // Create temp library root
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();

    // Seed a file at a "wrong" path
    let old_rel = "unsorted/track.flac";
    let old_abs = root.join(old_rel);
    fs::create_dir_all(old_abs.parent().unwrap()).await.unwrap();
    fs::write(&old_abs, b"audio").await.unwrap();

    // Create library + track in DB
    let lib = db.create_library("FLAC", root.to_str().unwrap(), "flac", None).await.unwrap();
    let track = db.upsert_track(suzuran_server::dal::UpsertTrack {
        library_id: lib.id,
        relative_path: old_rel.to_string(),
        file_hash: "abc".to_string(),
        title: Some("Comfortably Numb".to_string()),
        artist: Some("Pink Floyd".to_string()),
        albumartist: Some("Pink Floyd".to_string()),
        album: Some("The Wall".to_string()),
        tracknumber: Some("6".to_string()),
        discnumber: Some("1".to_string()),
        totaldiscs: Some("1".to_string()),
        totaltracks: Some("26".to_string()),
        date: Some("1979".to_string()),
        genre: None, composer: None, label: None, catalognumber: None,
        tags: serde_json::json!({
            "title": "Comfortably Numb",
            "albumartist": "Pink Floyd",
            "album": "The Wall",
            "tracknumber": "6",
            "discnumber": "1",
            "totaldiscs": "1",
            "date": "1979"
        }),
        duration_secs: None, bitrate: None, sample_rate: None, channels: None,
        has_embedded_art: false,
    }).await.unwrap();

    // Create an organization rule
    db.create_organization_rule(
        "Default",
        None, // global
        0,
        None, // match all
        "{albumartist}/{date} - {album}/{tracknumber:02} - {title}",
        true,
    ).await.unwrap();

    // Run the organize job handler
    let handler = OrganizeJobHandler;
    let payload = serde_json::to_value(OrganizePayload { track_id: track.id, dry_run: false }).unwrap();
    let result = handler.run(db.clone(), payload).await.unwrap();

    let expected_new = "Pink Floyd/1979 - The Wall/06 - Comfortably Numb";
    assert_eq!(result["new_path"], serde_json::json!(expected_new));
    assert_eq!(result["moved"], serde_json::json!(true));

    // File exists at new location
    assert!(root.join(expected_new).exists(), "file should be at new path");
    // File no longer at old location
    assert!(!old_abs.exists(), "file should no longer be at old path");

    // DB updated
    let updated_track = db.get_track(track.id).await.unwrap().unwrap();
    assert_eq!(updated_track.relative_path, expected_new);

    drop(dir);
}

#[tokio::test]
async fn organize_dry_run_does_not_move() {
    let db = make_db().await;
    let dir = tempfile::TempDir::new().unwrap();
    let root = dir.path();

    let old_rel = "track.flac";
    fs::write(root.join(old_rel), b"audio").await.unwrap();

    let lib = db.create_library("FLAC", root.to_str().unwrap(), "flac", None).await.unwrap();
    let track = db.upsert_track(suzuran_server::dal::UpsertTrack {
        library_id: lib.id,
        relative_path: old_rel.to_string(),
        file_hash: "xyz".to_string(),
        title: Some("Song".to_string()),
        albumartist: Some("Artist".to_string()),
        tags: serde_json::json!({"title":"Song","albumartist":"Artist","date":"2000"}),
        artist: None, album: None, tracknumber: Some("1".to_string()),
        discnumber: None, totaldiscs: None, totaltracks: None,
        date: Some("2000".to_string()), genre: None, composer: None,
        label: None, catalognumber: None,
        duration_secs: None, bitrate: None, sample_rate: None, channels: None,
        has_embedded_art: false,
    }).await.unwrap();

    db.create_organization_rule("Default", None, 0, None, "{albumartist}/{date}/{title}", true).await.unwrap();

    let handler = OrganizeJobHandler;
    let payload = serde_json::to_value(OrganizePayload { track_id: track.id, dry_run: true }).unwrap();
    let result = handler.run(db.clone(), payload).await.unwrap();

    assert_eq!(result["dry_run"], serde_json::json!(true));
    assert!(result["proposed_path"].is_string());
    // original file still at old path
    assert!(root.join(old_rel).exists());

    drop(dir);
}
```

Run: Expected compile error — `organize` module doesn't exist.

### Task 3: Add `OrganizePayload` to `src/jobs/mod.rs`

```rust
pub mod organize;

// existing ScanPayload …

#[derive(Debug, Serialize, Deserialize)]
pub struct OrganizePayload {
    pub track_id: i64,
    pub dry_run: bool,
}
```

### Task 4: Implement `OrganizeJobHandler`

Create `src/jobs/organize.rs`:

```rust
use std::{collections::HashMap, sync::Arc};
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

        // Build tag map from the track's full tags JSONB
        let tags: HashMap<String, String> = track.tags
            .as_object()
            .map(|obj| {
                obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect()
            })
            .unwrap_or_default();

        // Load rules (global + library-scoped), priority-sorted
        let rules_rows = db.list_organization_rules(Some(track.library_id)).await?;
        let rule_pairs: Vec<(Option<Value>, String)> = rules_rows
            .into_iter()
            .filter(|r| r.enabled)
            .map(|r| (r.conditions, r.path_template))
            .collect();

        let new_relative = apply_rules(&rule_pairs, &tags).ok_or_else(|| {
            AppError::BadRequest(format!("no matching rule for track {}", p.track_id))
        })?;

        if p.dry_run {
            return Ok(serde_json::json!({ "dry_run": true, "proposed_path": new_relative }));
        }

        // Move the file
        let old_abs = std::path::Path::new(&library.root_path).join(&track.relative_path);
        let new_abs = std::path::Path::new(&library.root_path).join(&new_relative);

        if let Some(parent) = new_abs.parent() {
            fs::create_dir_all(parent).await.map_err(|e| AppError::Internal(e.into()))?;
        }
        fs::rename(&old_abs, &new_abs).await.map_err(|e| AppError::Internal(e.into()))?;

        // Update DB
        db.update_track_path(track.id, &new_relative).await?;

        Ok(serde_json::json!({
            "moved": true,
            "old_path": track.relative_path,
            "new_path": new_relative,
        }))
    }
}
```

### Task 5: Register in scheduler

In `src/scheduler/mod.rs`, find the match on `job_type` and add:
```rust
"organize" => Arc::new(crate::jobs::organize::OrganizeJobHandler),
```

Check the existing structure in `src/scheduler/mod.rs` first — the exact pattern depends on
how handlers are dispatched. Follow whatever pattern `"scan"` uses.

### Task 6: Run tests

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: both organize_job tests pass.

### Task 7: Update filemap + commit

```bash
git add src/dal/ src/jobs/ src/scheduler/ tests/organize_job.rs tasks/codebase-filemap.md
git commit -m "feat(2.4): organize job handler (file move + DB path update)"
```

---

## Subphase 2.5 — Organization Rules API

**Files:**
- Create: `src/api/organization_rules.rs`
- Modify: `src/api/mod.rs` — mount the new router
- Create: `tests/organization_rules_api.rs`
- Modify: `tasks/codebase-filemap.md`

**Routes:**

| Method | Path | Auth | Description |
|--------|------|------|-------------|
| GET | `/api/v1/organization-rules` | user | List all rules (optional `?library_id=N`) |
| POST | `/api/v1/organization-rules` | admin | Create rule |
| GET | `/api/v1/organization-rules/:id` | user | Get one rule |
| PUT | `/api/v1/organization-rules/:id` | admin | Update rule |
| DELETE | `/api/v1/organization-rules/:id` | admin | Delete rule → 204 |
| POST | `/api/v1/organization-rules/preview` | admin | Dry-run against track list |
| POST | `/api/v1/organization-rules/apply` | admin | Enqueue organize jobs |

**Preview request/response:**
```json
// request
{ "library_id": 1, "track_ids": [42, 43] }

// response array
[{ "track_id": 42, "current_path": "unsorted/foo.flac",
   "proposed_path": "Air/1998 - Moon Safari/01 - La Femme d'Argent", "rule_matched": true }]
```

**Apply request/response:**
```json
// request
{ "library_id": 1, "track_ids": [42, 43] }

// response
{ "enqueued": 2 }
```

### Task 1: Write failing tests

Create `tests/organization_rules_api.rs`. Follow the `spawn_test_server()` pattern from `tests/auth.rs` — copy that helper into this file:

```rust
// (copy spawn_test_server() and test_webauthn() from tests/auth.rs)

async fn register_admin(base: &str) -> reqwest::Client {
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"admin","email":"admin@test.com","password":"password123"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"admin@test.com","password":"password123"}))
        .send().await.unwrap();
    client
}

#[tokio::test]
async fn org_rules_crud_via_api() {
    let base = spawn_test_server().await;
    let client = register_admin(&base).await;

    // Create a library first
    let lib_resp: serde_json::Value = client
        .post(format!("{base}/api/v1/libraries"))
        .json(&serde_json::json!({"name":"FLAC","root_path":"/tmp/flac","format":"flac"}))
        .send().await.unwrap().json().await.unwrap();
    let lib_id = lib_resp["id"].as_i64().unwrap();

    // Create rule
    let resp = client
        .post(format!("{base}/api/v1/organization-rules"))
        .json(&serde_json::json!({
            "name": "Default",
            "library_id": null,
            "priority": 0,
            "conditions": null,
            "path_template": "{albumartist}/{date} - {album}/{tracknumber:02} - {title}",
            "enabled": true
        }))
        .send().await.unwrap();
    assert_eq!(resp.status(), 201);
    let rule: serde_json::Value = resp.json().await.unwrap();
    let rule_id = rule["id"].as_i64().unwrap();

    // List — 1 rule
    let all: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/organization-rules"))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(all.len(), 1);

    // List filtered by library — returns global rule
    let filtered: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/organization-rules?library_id={lib_id}"))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(filtered.len(), 1);

    // Get one
    let one: serde_json::Value = client
        .get(format!("{base}/api/v1/organization-rules/{rule_id}"))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(one["name"], "Default");

    // Update
    let updated: serde_json::Value = client
        .put(format!("{base}/api/v1/organization-rules/{rule_id}"))
        .json(&serde_json::json!({
            "name": "Renamed", "priority": 5, "conditions": null,
            "path_template": "{title}", "enabled": false
        }))
        .send().await.unwrap().json().await.unwrap();
    assert_eq!(updated["name"], "Renamed");
    assert!(!updated["enabled"].as_bool().unwrap());

    // Delete
    let del_status = client
        .delete(format!("{base}/api/v1/organization-rules/{rule_id}"))
        .send().await.unwrap().status();
    assert_eq!(del_status, 204);

    let after: Vec<serde_json::Value> = client
        .get(format!("{base}/api/v1/organization-rules"))
        .send().await.unwrap().json().await.unwrap();
    assert!(after.is_empty());
}

#[tokio::test]
async fn create_rule_requires_admin() {
    let base = spawn_test_server().await;
    // Register second (non-admin) user
    let client = reqwest::Client::builder().cookie_store(true).build().unwrap();
    // Admin must exist for second user to register
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"admin","email":"admin@test.com","password":"pw123456"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/register"))
        .json(&serde_json::json!({"username":"user2","email":"user2@test.com","password":"pw123456"}))
        .send().await.unwrap();
    client.post(format!("{base}/api/v1/auth/login"))
        .json(&serde_json::json!({"email":"user2@test.com","password":"pw123456"}))
        .send().await.unwrap();
    let resp = client
        .post(format!("{base}/api/v1/organization-rules"))
        .json(&serde_json::json!({"name":"x","priority":0,"conditions":null,
            "path_template":"{title}","enabled":true}))
        .send().await.unwrap();
    assert_eq!(resp.status(), 403);
}
```

Run: Expected compile error / 404 — endpoint not registered.

### Task 2: Implement `src/api/organization_rules.rs`

Follow `src/api/libraries.rs` exactly:

```rust
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;

use crate::{
    api::middleware::{admin::AdminUser, auth::AuthUser},
    error::AppError,
    jobs::OrganizePayload,
    models::OrganizationRule,
    state::AppState,
};

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/", get(list_rules).post(create_rule))
        .route("/preview", post(preview))
        .route("/apply", post(apply))
        .route("/:id", get(get_rule).put(update_rule).delete(delete_rule))
}

#[derive(Deserialize)]
struct ListQuery {
    library_id: Option<i64>,
}

async fn list_rules(
    State(state): State<AppState>,
    _auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> Result<Json<Vec<OrganizationRule>>, AppError> {
    Ok(Json(state.db.list_organization_rules(q.library_id).await?))
}

async fn get_rule(
    State(state): State<AppState>,
    _auth: AuthUser,
    Path(id): Path<i64>,
) -> Result<Json<OrganizationRule>, AppError> {
    state.db.get_organization_rule(id).await?
        .ok_or_else(|| AppError::NotFound(format!("rule {id} not found")))
        .map(Json)
}

#[derive(Deserialize)]
struct CreateRuleRequest {
    name: String,
    library_id: Option<i64>,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: String,
    enabled: bool,
}

async fn create_rule(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<CreateRuleRequest>,
) -> Result<(StatusCode, Json<OrganizationRule>), AppError> {
    let rule = state.db.create_organization_rule(
        &body.name, body.library_id, body.priority,
        body.conditions, &body.path_template, body.enabled,
    ).await?;
    Ok((StatusCode::CREATED, Json(rule)))
}

#[derive(Deserialize)]
struct UpdateRuleRequest {
    name: String,
    priority: i32,
    conditions: Option<serde_json::Value>,
    path_template: String,
    enabled: bool,
}

async fn update_rule(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
    Json(body): Json<UpdateRuleRequest>,
) -> Result<Json<OrganizationRule>, AppError> {
    state.db.update_organization_rule(
        id, &body.name, body.priority,
        body.conditions, &body.path_template, body.enabled,
    ).await?
    .ok_or_else(|| AppError::NotFound(format!("rule {id} not found")))
    .map(Json)
}

async fn delete_rule(
    State(state): State<AppState>,
    _admin: AdminUser,
    Path(id): Path<i64>,
) -> Result<StatusCode, AppError> {
    state.db.delete_organization_rule(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct PreviewApplyRequest {
    library_id: i64,
    track_ids: Vec<i64>,
}

async fn preview(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<PreviewApplyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    use crate::organizer::rules::apply_rules;
    use std::collections::HashMap;

    let rules = state.db.list_organization_rules(Some(body.library_id)).await?;
    let rule_pairs: Vec<(Option<serde_json::Value>, String)> = rules.into_iter()
        .filter(|r| r.enabled)
        .map(|r| (r.conditions, r.path_template))
        .collect();

    let mut results = Vec::new();
    for track_id in &body.track_ids {
        if let Some(track) = state.db.get_track(*track_id).await? {
            let tags: HashMap<String, String> = track.tags
                .as_object()
                .map(|obj| obj.iter()
                    .filter_map(|(k, v)| v.as_str().map(|s| (k.clone(), s.to_string())))
                    .collect())
                .unwrap_or_default();
            let proposed = apply_rules(&rule_pairs, &tags);
            results.push(serde_json::json!({
                "track_id": track_id,
                "current_path": track.relative_path,
                "proposed_path": proposed,
                "rule_matched": proposed.is_some(),
            }));
        }
    }
    Ok(Json(serde_json::Value::Array(results)))
}

async fn apply(
    State(state): State<AppState>,
    _admin: AdminUser,
    Json(body): Json<PreviewApplyRequest>,
) -> Result<Json<serde_json::Value>, AppError> {
    let mut enqueued = 0i64;
    for track_id in &body.track_ids {
        state.db.enqueue_job(
            "organize",
            serde_json::to_value(OrganizePayload { track_id: *track_id, dry_run: false }).unwrap(),
            0,
        ).await?;
        enqueued += 1;
    }
    Ok(Json(serde_json::json!({ "enqueued": enqueued })))
}
```

### Task 3: Mount in `src/api/mod.rs`

Find where other routers are nested (`.nest("/libraries", ...)`) and add:
```rust
.nest("/organization-rules", organization_rules::router())
```

Add `mod organization_rules;` at the top of the file.

### Task 4: Run tests

```bash
docker buildx build --progress=plain -t suzuran:dev .
```

Expected: all `organization_rules_api` tests pass.

### Task 5: Update filemap + commit

```bash
git add src/api/ tests/organization_rules_api.rs tasks/codebase-filemap.md
git commit -m "feat(2.5): organization rules API (CRUD, preview, apply)"
```

---

## Subphase 2.6 — Library Management UI

**Files:**
- Modify: `ui/src/api/libraries.ts` — add create, update, delete
- Create: `ui/src/components/LibraryFormModal.tsx`
- Modify: `ui/src/components/LibraryTree.tsx` — real data, hierarchy, admin actions
- Modify: `ui/src/pages/LibraryPage.tsx` — useQuery instead of static data
- Modify: `tasks/codebase-filemap.md`

### Task 1: Extend `ui/src/api/libraries.ts`

Read the current file first, then add:

```typescript
import type { Library } from '../types';  // add this type if it doesn't exist

export interface CreateLibraryRequest {
  name: string;
  root_path: string;
  format: string;
  parent_library_id?: number | null;
}

export const createLibrary = (data: CreateLibraryRequest) =>
  api.post<Library>('/libraries', data).then(r => r.data);

export const updateLibrary = (id: number, data: {
  name: string;
  scan_enabled: boolean;
  scan_interval_secs: number;
  auto_transcode_on_ingest: boolean;
  auto_organize_on_ingest: boolean;
}) => api.put<Library>(`/libraries/${id}`, data).then(r => r.data);

export const deleteLibrary = (id: number) =>
  api.delete(`/libraries/${id}`);
```

If a `Library` type isn't already defined in `ui/src/types.ts`, create it matching the backend model.

### Task 2: Create `LibraryFormModal.tsx`

```tsx
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createLibrary, updateLibrary, listLibraries } from '../api/libraries';
import type { Library } from '../types';

interface Props {
  existing?: Library;
  onClose: () => void;
}

const FORMATS = ['flac', 'aac', 'mp3', 'opus', 'wav'];

export function LibraryFormModal({ existing, onClose }: Props) {
  const qc = useQueryClient();
  const { data: allLibs = [] } = useQuery({ queryKey: ['libraries'], queryFn: listLibraries });

  const [name, setName] = useState(existing?.name ?? '');
  const [rootPath, setRootPath] = useState(existing?.root_path ?? '');
  const [format, setFormat] = useState(existing?.format ?? 'flac');
  const [parentId, setParentId] = useState<number | null>(existing?.parent_library_id ?? null);

  const mutation = useMutation({
    mutationFn: () => existing
      ? updateLibrary(existing.id, {
          name, scan_enabled: existing.scan_enabled,
          scan_interval_secs: existing.scan_interval_secs,
          auto_transcode_on_ingest: existing.auto_transcode_on_ingest,
          auto_organize_on_ingest: existing.auto_organize_on_ingest,
        })
      : createLibrary({ name, root_path: rootPath, format, parent_library_id: parentId }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['libraries'] }); onClose(); },
  });

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-[var(--surface-1)] border border-[var(--border)] rounded-lg p-6 w-[480px]">
        <h2 className="text-[var(--text-primary)] font-semibold mb-4">
          {existing ? 'Edit Library' : 'Add Library'}
        </h2>

        <div className="space-y-3">
          <div>
            <label className="block text-xs text-[var(--text-muted)] mb-1">Name</label>
            <input
              className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5
                         text-[var(--text-primary)] text-sm focus:outline-none focus:border-[var(--accent)]"
              value={name} onChange={e => setName(e.target.value)}
            />
          </div>

          {!existing && (
            <>
              <div>
                <label className="block text-xs text-[var(--text-muted)] mb-1">Root Path</label>
                <input
                  className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5
                             text-[var(--text-primary)] text-sm font-mono focus:outline-none focus:border-[var(--accent)]"
                  value={rootPath} onChange={e => setRootPath(e.target.value)}
                  placeholder="/music/flac"
                />
              </div>
              <div>
                <label className="block text-xs text-[var(--text-muted)] mb-1">Format</label>
                <select
                  className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5
                             text-[var(--text-primary)] text-sm focus:outline-none"
                  value={format} onChange={e => setFormat(e.target.value)}
                >
                  {FORMATS.map(f => <option key={f} value={f}>{f.toUpperCase()}</option>)}
                </select>
              </div>
              <div>
                <label className="block text-xs text-[var(--text-muted)] mb-1">
                  Parent Library <span className="text-[var(--text-faint)]">(optional)</span>
                </label>
                <select
                  className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5
                             text-[var(--text-primary)] text-sm focus:outline-none"
                  value={parentId ?? ''} onChange={e => setParentId(e.target.value ? Number(e.target.value) : null)}
                >
                  <option value="">None (source library)</option>
                  {allLibs.map(l => <option key={l.id} value={l.id}>{l.name}</option>)}
                </select>
              </div>
            </>
          )}
        </div>

        {mutation.isError && (
          <p className="mt-3 text-xs text-red-400">Failed to save. Check the values and try again.</p>
        )}

        <div className="flex gap-2 justify-end mt-5">
          <button
            onClick={onClose}
            className="px-3 py-1.5 text-sm text-[var(--text-muted)] hover:text-[var(--text-primary)]"
          >
            Cancel
          </button>
          <button
            onClick={() => mutation.mutate()}
            disabled={!name.trim() || mutation.isPending}
            className="px-4 py-1.5 text-sm bg-[var(--accent)] text-white rounded
                       hover:opacity-90 disabled:opacity-40"
          >
            {mutation.isPending ? 'Saving…' : existing ? 'Save' : 'Add Library'}
          </button>
        </div>
      </div>
    </div>
  );
}
```

### Task 3: Update `LibraryTree.tsx`

Read the existing file first. Replace the static/stub content with:
- `useQuery(['libraries'], listLibraries)` for data
- Sort: top-level first (no parent), then indent children
- Show edit icon (pencil) and delete icon (trash) on hover for admin users
- "+" button in tree header for admin
- Empty state: "No libraries — click + to add one"

For delete: call `deleteLibrary(id)` via `useMutation`, show confirm dialog (`window.confirm`),
invalidate `['libraries']` on success.

Pass `isAdmin: boolean` prop. Wire the "+" and edit buttons to open `LibraryFormModal`.

### Task 4: Update `LibraryPage.tsx`

Read the existing file first. Wire up `useAuth()` to pass `isAdmin` into `LibraryTree`.
Pass the selected library's `id` down to the track list pane.

### Task 5: Build and verify in browser

```bash
docker buildx build --progress=plain -t suzuran:dev .
docker compose up --build -d
```

Visit http://localhost:3000, log in as admin. Verify:
- Library tree shows (empty state if no libraries)
- "+" button opens create modal
- Creating a library → it appears in the tree
- Edit modal opens and saves
- Delete removes it from the tree

### Task 6: Commit

```bash
git add ui/src/
git commit -m "feat(2.6): library management UI (create, edit, delete, hierarchy)"
```

---

## Subphase 2.7 — Organization Rules UI

**Files:**
- Create: `ui/src/api/organizationRules.ts`
- Create: `ui/src/pages/OrganizationPage.tsx`
- Create: `ui/src/components/RuleEditor.tsx`
- Create: `ui/src/components/TemplatePreview.tsx`
- Modify: `ui/src/App.tsx` — add `/organization` route
- Modify: `ui/src/components/TopNav.tsx` — add Organization nav link (admin only)
- Modify: `tasks/codebase-filemap.md`

### Task 1: Create `ui/src/api/organizationRules.ts`

```typescript
import api from './client';

export interface OrgRule {
  id: number;
  name: string;
  library_id: number | null;
  priority: number;
  conditions: unknown | null;
  path_template: string;
  enabled: boolean;
  created_at: string;
}

export interface CreateRuleRequest {
  name: string;
  library_id: number | null;
  priority: number;
  conditions: unknown | null;
  path_template: string;
  enabled: boolean;
}

export const listRules = (library_id?: number) =>
  api.get<OrgRule[]>('/organization-rules', { params: library_id != null ? { library_id } : {} })
     .then(r => r.data);

export const createRule = (data: CreateRuleRequest) =>
  api.post<OrgRule>('/organization-rules', data).then(r => r.data);

export const updateRule = (id: number, data: Omit<CreateRuleRequest, 'library_id'>) =>
  api.put<OrgRule>(`/organization-rules/${id}`, data).then(r => r.data);

export const deleteRule = (id: number) => api.delete(`/organization-rules/${id}`);
```

### Task 2: Create `TemplatePreview.tsx`

```tsx
import { useMemo } from 'react';

const SAMPLE = {
  title: 'Comfortably Numb', artist: 'Pink Floyd',
  albumartist: 'Pink Floyd', album: 'The Wall',
  tracknumber: '6', discnumber: '2', totaldiscs: '2',
  date: '1979', genre: 'Rock', label: 'Harvest',
};

function renderTemplate(template: string, tags: Record<string, string>): string {
  // Mirror the Rust logic for live preview (client-side)
  return template.replace(/\{([^}]+)\}/g, (_, token) => {
    if (token === 'discfolder') {
      const total = parseInt(tags['totaldiscs'] ?? '0');
      if (total > 1) return `Disc ${tags['discnumber'] ?? '1'}/`;
      return '';
    }
    if (token.includes('|')) {
      const [field, fallback] = token.split('|', 2);
      return (tags[field] ?? '').trim() || fallback;
    }
    if (token.includes(':')) {
      const [field, fmt] = token.split(':', 2);
      const width = parseInt(fmt);
      return String(parseInt(tags[field] ?? '0')).padStart(width, '0');
    }
    return tags[token] ?? '';
  });
}

interface Props { template: string }

export function TemplatePreview({ template }: Props) {
  const preview = useMemo(() => renderTemplate(template, SAMPLE), [template]);
  return (
    <div className="mt-1.5 text-xs font-mono text-[var(--text-muted)] bg-[var(--surface-0)]
                    border border-[var(--border)] rounded px-3 py-1.5 truncate">
      {preview || <span className="italic text-[var(--text-faint)]">— preview —</span>}
    </div>
  );
}
```

### Task 3: Create `RuleEditor.tsx`

Modal form with fields: Name, Library (select), Priority, Path Template (+ live preview),
Enabled toggle. No conditions builder in v1.0 — conditions are null (match all) unless the
user is an advanced operator who can set them via the API directly.

```tsx
import { useState } from 'react';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { createRule, updateRule, type OrgRule } from '../api/organizationRules';
import { listLibraries } from '../api/libraries';
import { TemplatePreview } from './TemplatePreview';

interface Props {
  existing?: OrgRule;
  onClose: () => void;
}

export function RuleEditor({ existing, onClose }: Props) {
  const qc = useQueryClient();
  const { data: libs = [] } = useQuery({ queryKey: ['libraries'], queryFn: listLibraries });

  const [name, setName] = useState(existing?.name ?? '');
  const [libraryId, setLibraryId] = useState<number | null>(existing?.library_id ?? null);
  const [priority, setPriority] = useState(existing?.priority ?? 0);
  const [template, setTemplate] = useState(existing?.path_template ?? '');
  const [enabled, setEnabled] = useState(existing?.enabled ?? true);

  const mutation = useMutation({
    mutationFn: () => existing
      ? updateRule(existing.id, { name, priority, conditions: existing.conditions, path_template: template, enabled })
      : createRule({ name, library_id: libraryId, priority, conditions: null, path_template: template, enabled }),
    onSuccess: () => { qc.invalidateQueries({ queryKey: ['org-rules'] }); onClose(); },
  });

  return (
    <div className="fixed inset-0 bg-black/60 flex items-center justify-center z-50">
      <div className="bg-[var(--surface-1)] border border-[var(--border)] rounded-lg p-6 w-[560px]">
        <h2 className="text-[var(--text-primary)] font-semibold mb-4">
          {existing ? 'Edit Rule' : 'New Rule'}
        </h2>
        <div className="space-y-3">
          <div>
            <label className="block text-xs text-[var(--text-muted)] mb-1">Name</label>
            <input className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5 text-sm text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
              value={name} onChange={e => setName(e.target.value)} />
          </div>
          {!existing && (
            <div>
              <label className="block text-xs text-[var(--text-muted)] mb-1">
                Library <span className="text-[var(--text-faint)]">(None = global)</span>
              </label>
              <select className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5 text-sm text-[var(--text-primary)] focus:outline-none"
                value={libraryId ?? ''} onChange={e => setLibraryId(e.target.value ? Number(e.target.value) : null)}>
                <option value="">Global (all libraries)</option>
                {libs.map(l => <option key={l.id} value={l.id}>{l.name}</option>)}
              </select>
            </div>
          )}
          <div>
            <label className="block text-xs text-[var(--text-muted)] mb-1">Priority</label>
            <input type="number" className="w-24 bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5 text-sm text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
              value={priority} onChange={e => setPriority(Number(e.target.value))} />
            <span className="ml-2 text-xs text-[var(--text-muted)]">Lower = higher priority</span>
          </div>
          <div>
            <label className="block text-xs text-[var(--text-muted)] mb-1">Path Template</label>
            <input className="w-full bg-[var(--surface-2)] border border-[var(--border)] rounded px-3 py-1.5 text-sm font-mono text-[var(--text-primary)] focus:outline-none focus:border-[var(--accent)]"
              value={template} onChange={e => setTemplate(e.target.value)}
              placeholder="{albumartist}/{date} - {album}/{tracknumber:02} - {title}" />
            <TemplatePreview template={template} />
          </div>
          <div className="flex items-center gap-2">
            <input type="checkbox" id="enabled" checked={enabled} onChange={e => setEnabled(e.target.checked)} className="accent-[var(--accent)]" />
            <label htmlFor="enabled" className="text-sm text-[var(--text-primary)]">Enabled</label>
          </div>
        </div>
        {mutation.isError && <p className="mt-3 text-xs text-red-400">Failed to save.</p>}
        <div className="flex gap-2 justify-end mt-5">
          <button onClick={onClose} className="px-3 py-1.5 text-sm text-[var(--text-muted)] hover:text-[var(--text-primary)]">Cancel</button>
          <button onClick={() => mutation.mutate()} disabled={!name.trim() || !template.trim() || mutation.isPending}
            className="px-4 py-1.5 text-sm bg-[var(--accent)] text-white rounded hover:opacity-90 disabled:opacity-40">
            {mutation.isPending ? 'Saving…' : 'Save Rule'}
          </button>
        </div>
      </div>
    </div>
  );
}
```

### Task 4: Create `OrganizationPage.tsx`

```tsx
import { useState } from 'react';
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { listRules, deleteRule, type OrgRule } from '../api/organizationRules';
import { RuleEditor } from '../components/RuleEditor';

export default function OrganizationPage() {
  const qc = useQueryClient();
  const { data: rules = [], isLoading } = useQuery({ queryKey: ['org-rules'], queryFn: () => listRules() });
  const [editing, setEditing] = useState<OrgRule | null | 'new'>(null);

  const deleteMutation = useMutation({
    mutationFn: (id: number) => deleteRule(id),
    onSuccess: () => qc.invalidateQueries({ queryKey: ['org-rules'] }),
  });

  return (
    <div className="flex-1 p-6">
      <div className="flex items-center justify-between mb-4">
        <h1 className="text-[var(--text-primary)] font-semibold text-lg">Organization Rules</h1>
        <button onClick={() => setEditing('new')}
          className="px-3 py-1.5 text-sm bg-[var(--accent)] text-white rounded hover:opacity-90">
          + New Rule
        </button>
      </div>

      {isLoading ? (
        <p className="text-[var(--text-muted)] text-sm">Loading…</p>
      ) : rules.length === 0 ? (
        <div className="text-center py-16">
          <p className="text-[var(--text-muted)] text-sm mb-3">No organization rules defined.</p>
          <p className="text-[var(--text-faint)] text-xs">
            Rules determine how files are named and organized on disk.
          </p>
        </div>
      ) : (
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-[var(--border)] text-[var(--text-muted)] text-xs">
              <th className="text-left pb-2 pr-4 font-medium">Priority</th>
              <th className="text-left pb-2 pr-4 font-medium">Name</th>
              <th className="text-left pb-2 pr-4 font-medium">Library</th>
              <th className="text-left pb-2 pr-4 font-medium">Template</th>
              <th className="text-left pb-2 pr-4 font-medium">Enabled</th>
              <th className="pb-2"></th>
            </tr>
          </thead>
          <tbody>
            {rules.map(rule => (
              <tr key={rule.id} className="border-b border-[var(--border-faint)] hover:bg-[var(--surface-2)]">
                <td className="py-2 pr-4 text-[var(--text-muted)]">{rule.priority}</td>
                <td className="py-2 pr-4 text-[var(--text-primary)] font-medium">{rule.name}</td>
                <td className="py-2 pr-4 text-[var(--text-muted)]">{rule.library_id == null ? 'Global' : `#${rule.library_id}`}</td>
                <td className="py-2 pr-4 font-mono text-[var(--text-muted)] max-w-xs truncate">{rule.path_template}</td>
                <td className="py-2 pr-4">
                  <span className={`text-xs px-1.5 py-0.5 rounded ${rule.enabled ? 'bg-green-900/40 text-green-400' : 'bg-[var(--surface-2)] text-[var(--text-faint)]'}`}>
                    {rule.enabled ? 'on' : 'off'}
                  </span>
                </td>
                <td className="py-2 pl-2">
                  <div className="flex gap-2 justify-end">
                    <button onClick={() => setEditing(rule)} className="text-[var(--text-muted)] hover:text-[var(--text-primary)] text-xs">Edit</button>
                    <button
                      onClick={() => { if (window.confirm(`Delete rule "${rule.name}"?`)) deleteMutation.mutate(rule.id); }}
                      className="text-[var(--text-muted)] hover:text-red-400 text-xs"
                    >Delete</button>
                  </div>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {editing != null && (
        <RuleEditor
          existing={editing === 'new' ? undefined : editing}
          onClose={() => setEditing(null)}
        />
      )}
    </div>
  );
}
```

### Task 5: Wire up route in `App.tsx`

Read `App.tsx` first. Add:
```tsx
import OrganizationPage from './pages/OrganizationPage';
// inside the router:
<Route path="/organization" element={<OrganizationPage />} />
```

### Task 6: Add nav link in `TopNav.tsx`

Read `TopNav.tsx` first. Add an Organization link visible only to admin users:
```tsx
{user?.role === 'admin' && (
  <NavLink to="/organization" className={...}>Organization</NavLink>
)}
```

### Task 7: Build and verify

```bash
docker buildx build --progress=plain -t suzuran:dev .
docker compose up --build -d
```

Visit http://localhost:3000, log in as admin. Verify:
- "Organization" link visible in nav
- Organization page loads with empty state
- "New Rule" opens the editor modal
- Template field shows live preview as you type
- Saving creates a rule in the table
- Edit and delete work
- Non-admin user does not see the nav link

### Task 8: Update filemap + commit

```bash
git add ui/src/ tasks/codebase-filemap.md
git commit -m "feat(2.7): organization rules UI (rule editor, template preview)"
```

---

## Phase 2 Completion

After all 7 subphases merge cleanly:

```bash
# Merge subphase branches → 0.2 (one per subphase)
git checkout 0.2
git merge --no-ff 0.2.1 -m "merge: 2.1 organization rules migration & DAL"
# ...repeat for 0.2.2 through 0.2.7...

# Merge 0.2 → main and tag
git checkout main
git merge --no-ff 0.2 -m "release: v0.2.0 organization engine"
git tag v0.2.0
```

**Phase 2 deliverables:**
- `organization_rules` DB table (Postgres + SQLite)
- Full DAL CRUD for organization rules
- Path template engine with all token types
- Condition expression tree evaluator
- Organize job handler (file move + DB update)
- REST API: CRUD + preview + apply
- Library management UI (create / edit / delete)
- Organization rules UI with live template preview
