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
