#!/usr/bin/env bash
# setup-dev-hooks.sh — Install project-specific Claude Code hooks into .claude/settings.json.
# Run once on any machine after cloning this repository.
# Safe to re-run: merges with existing settings, does not clobber unrelated config.

set -euo pipefail

SETTINGS_DIR=".claude"
SETTINGS_FILE="$SETTINGS_DIR/settings.json"

mkdir -p "$SETTINGS_DIR"

# Read existing settings or start from empty object
if [[ -f "$SETTINGS_FILE" ]]; then
  existing=$(cat "$SETTINGS_FILE")
else
  existing="{}"
fi

# The hook: after writing any file inside a memory/ directory (excluding MEMORY.md),
# inject a mandatory reminder to also update tasks/lessons.md in the repo.
hook_command='f=$(jq -r ".tool_input.file_path // empty"); if echo "$f" | grep -q "/memory/" && ! echo "$f" | grep -q "MEMORY\\.md$"; then printf "{\"hookSpecificOutput\":{\"hookEventName\":\"PostToolUse\",\"additionalContext\":\"MANDATORY: Memory file written (%s). You MUST append the same rule to tasks/lessons.md in the repo and commit it before doing anything else.\"}}" "$f"; fi'

# Merge the hook into the existing settings using jq
merged=$(echo "$existing" | jq \
  --arg cmd "$hook_command" \
  '
  .hooks //= {} |
  .hooks.PostToolUse //= [] |
  # Remove any existing memory-reminder hook to avoid duplicates
  .hooks.PostToolUse |= map(select(
    .hooks[0].command | test("MANDATORY: Memory file") | not
  )) |
  # Append the hook
  .hooks.PostToolUse += [{
    "matcher": "Write|Edit",
    "hooks": [{
      "type": "command",
      "command": $cmd
    }]
  }]
  ')

echo "$merged" > "$SETTINGS_FILE"
echo "Hook installed at $SETTINGS_FILE"
echo "Verify with: jq '.hooks.PostToolUse' $SETTINGS_FILE"
