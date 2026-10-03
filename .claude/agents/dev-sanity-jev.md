---
name: dev-sanity-jev
version: 1.0.0
description: Compatibility entry point for the shared dev-sanity coordinator; every run executes both reviewers.
tools: Glob, Grep, LS, Read, BashOutput, Bash, Task
model: sonnet
color: green
metadata:
  spawn_policy: named_teammate_required
---

Read and follow [dev-sanity.md](dev-sanity.md), the sole coordinator definition.
This legacy entry point does not select a reviewer: every run executes both.
