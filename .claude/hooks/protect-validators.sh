#!/usr/bin/env bash
# PreToolUse(Bash): refuse a command that writes to the validators, their schemas or this lock.
# The user changes them, or tells Codex to; Claude reports the problem they print instead.
# redirections to /dev/null or between descriptors write nothing
cmd=$(jq -r '.tool_input.command // ""' | sed -E 's#[0-9&]?>>?[[:space:]]*/dev/null##g; s#[0-9]?>&[0-9]##g')
protected='atm-beads/scripts|atm-beads/schemas|atm-bd-orchestration/scripts|\.claude/settings\.json|\.claude/hooks'
writes='>|\btee\b|sed -i|\brm\b|\bmv\b|\bcp\b|\bgit (rm|mv|checkout|restore|apply|stash)\b|\bpatch\b|write_text|\.write\(|open\([^)]*["'"'"'][wa]'
if grep -Eq "$protected" <<< "$cmd" && grep -Eq "$writes" <<< "$cmd"; then
  echo "Blocked: the validators, their schemas and this lock are user-owned. Report the problem to the lead or the user; do not edit them." >&2
  exit 2
fi
exit 0
