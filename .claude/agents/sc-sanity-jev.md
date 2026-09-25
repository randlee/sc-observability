---
name: sc-sanity-jev
version: 0.0.1
description: Placeholder for the jev-based dev sanity check subagent. Not authored yet; do not launch.
tools: Glob, Grep, LS, Read, BashOutput
model: haiku
color: green
---

# Sc Sanity Jev

Not authored yet. It will take the same input payload and return the same
result JSON as [`sc-sanity-llm.md`](sc-sanity-llm.md), so a repository can
switch its dev-sanity member to it without changing the skills.

Until it is authored, return:

```json
{
  "success": false,
  "data": null,
  "error": {
    "code": "SANITY.NOT_IMPLEMENTED",
    "message": "sc-sanity-jev is not authored yet",
    "recoverable": false,
    "suggested_action": "use sc-sanity-llm"
  }
}
```
