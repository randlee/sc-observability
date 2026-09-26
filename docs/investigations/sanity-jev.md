---
id: sanity-jev
status: complete
branch: feature/sanity-jev
worktree: /Users/randlee/github/sc-observability-worktrees/feature/sanity-jev
task_id: obs-sanity-jev-1
investigated: 2026-09-25
adapter_status: draft-unvalidated
---

# Jev for dev sanity checks

## Recommendation

Keep the current LLM checker active. Pilot Jev as a decision service behind a
small wrapper, initially with an LLM subagent preparing evidence. Replacing the
whole subagent is not justified yet: Jev cannot browse a repository, run lint,
or compose our findings. TypeSafe explicitly distinguishes it from coding-agent
LLMs. Its documented name is a **System One model**, rather than “layer 1.”
[TypeSafe coding-agent guidance](https://docs.typesafe.ai/introduction/coding-agents)

This investigation and the proposed prompts are complete; the adapter is a
**draft**, not a validated replacement. No Jev inference request was made:
`TYPESAFE_API_KEY` is absent in the session. Only its presence was checked, not
credential contents. Model accuracy, end-to-end latency and billed cost on this
repository are **unverified**.

The task assignment is the spec, with the sanity role and three agent prompts
read at parent commit `bfb5bfb`. The assignment's literal `none: this assignment
is the spec...` sprint-path/frontmatter bullets are template artifacts, not a
filesystem path. This report supplies the completion metadata and is linked in
`docs/project-plan.md`.

## What the service provides

Vendor-documented behavior below — **untested: no API key**.

Jev evaluates supplied state with typed questions. Choice selects an option,
Score evaluates ordered levels, and Noul evaluates a yes/no proposition.
Questions share state but are evaluated independently. Narrow questions can be
combined in application logic; a single “is this sprint done?” question hides too
many checks. [Introduction](https://docs.typesafe.ai/introduction)

| Interface | Documented use |
| --- | --- |
| HTTP | `POST https://api.typesafe.ai/v1/systemone`; bearer API key; JSON `model`, `state`, `questions`; response `model`, `answers`, `usage` |
| Python | `pip install typesafe-sdk`; `typesafe_sdk.TypeSafeClient` or `AsyncTypeSafeClient`; `system_one(...)` |
| JavaScript | Official `@typesafe-ai/sdk`; see the linked SDK documentation |
| CLI / UI | Docs show curl and a console Playground. A standalone official `jev` CLI was not established by this investigation; don't invent one. |

Keys are issued through the TypeSafe dashboard; the SDK reads
`TYPESAFE_API_KEY`. Python requires 3.10 or newer. The executable helper uses Python standard-library HTTPS, so no SDK package
is required. [Quick start](https://docs.typesafe.ai/introduction/quickstart),
[Python SDK](https://docs.typesafe.ai/sdk/python),
[Client SDKs](https://docs.typesafe.ai/sdk),
[HTTP reference](https://docs.typesafe.ai/api)

## Economics and operating limits

Vendor prices, limits and latency claims below — **untested: no API key**.

As documented on 2026-09-25, `jev-1.13.0` costs $0.042 per million input tokens;
output tokens are free. A 20,000-input-token call would therefore cost $0.00084,
before retries or additional batches (arithmetic estimate, not a measured bill).
The listed limits are 250,000 tokens/second and 1,200 requests/minute, explicitly
subject to change. The request budget is 64k tokens overall and 32k for state
plus the longest question. Input is text/JSON, not images or binaries. Pin the
version for a trial: aliases can move. Confirm account limits before rollout.
[Models and pricing](https://docs.typesafe.ai/models)

The homepage demonstrates 0.114 seconds for one example workflow. That is a
vendor demonstration, not a latency guarantee or our benchmark. Repository
collection, lint and evidence preparation may dominate total time; p50/p95
latency for this workload and an availability SLA remain unverified.
[TypeSafe homepage](https://typesafe.ai/)

The API documents 401, 422, 429 and 529 errors. Handle auth/schema failures
without retry; cap retries for throttling/overload and respect server retry
advice. A timeout or exhausted retry is “cannot run”, never PASS.
[HTTP reference](https://docs.typesafe.ai/api)

## Compatibility with our contract

Proposed mapping of Jev behavior — **untested: no API key**. Local field-name
compatibility is checked separately.

The [payload and result](../../.claude/agents/sc-sanity-llm.md) ("Inputs", "Output Format")
remain authoritative. `roles.dev-sanity` resolves to `obs-sanity`; its startup
directive currently launches `sc-sanity-llm`. A future switch changes only the
startup directive path in `.atm.toml`, after the alternative prompts ship.

| Check / output | Owner in the proposed adapter |
| --- | --- |
| Payload fields, identity and exact SHA | Wrapper validates and echoes them; the model cannot choose them |
| Skipped deliverable / acceptance criterion | Wrapper enumerates every criterion and retrieves relevant committed evidence; Jev classifies each supported/missing/uncertain proposition |
| Obvious code error | Wrapper supplies localized candidate defects and evidence; Jev confirms or rejects candidates; this cannot prove unenumerated defects absent |
| Lint | Local process runs `lint_command`; real exit status and parsed diagnostics determine lint findings |
| Finding kind/file/line/issue | Wrapper builds these from validated candidates and committed locations, not generated Jev prose |
| Final Result envelope | Wrapper serializes the exact role schema; model answers are an internal intermediate |

The input JSON can be included in API state, but paths alone convey no source
content. The API response is not our Result. We must explicitly transform it.
For a missing file there is no genuine target line: return an inconclusive error
unless another real committed location supports the finding; never invent line 1.

The [proposed checker](../../.claude/agents/sc-sanity-jev.md) preserves all Payload
and Result field names and the error envelope. The
[alternative directive](../../.claude/agents/dev-sanity-jev.md) is needed because
the current directive hardcodes `sc-sanity-llm`. The role, templates, member
mapping and active `.atm.toml` are unchanged.

## Glue: script versus subagent

Jev-dependent behavior below — **untested: no API key**.

A deterministic script is the eventual economical design: collect immutable
Git evidence, run lint, prepare questions, call Jev and serialize the envelope.
It needs an explicit criterion-to-evidence strategy; regex alone cannot turn
arbitrary prose requirements into complete checks. The implemented `scripts/jev_client.py` handles transport, response validation
and a startup probe; evidence collection and final Result construction still
belong to the wrapper. A fully deterministic end-to-end checker is not claimed.

For the initial draft, an LLM wrapper follows `sc-sanity-jev.md` to prepare
candidates and call the typed API. This still spends LLM tokens and is **not a
full subagent replacement**. It makes the uncertainty measurable without
pretending the model has tool access. Extract stable collection, API and
serialization steps into a script only after the pilot shows the approach is
worthwhile. Do not install a TypeSafe skill or change runtime configuration as
part of this investigation.

Jev's documented weak areas include indirection, numerical reasoning,
irrelevant large context and adversarial state. It does not generate free-form
text. A broad “no errors” answer cannot establish full repository coverage.
Keep arithmetic, evidence locations, command outcomes and completeness checks
in the wrapper; insufficient context must produce “cannot run.”
[Known limitations](https://docs.typesafe.ai/model-jaggedness/jev-1.13)

Choice confidence summarizes its probability distribution, not a proof of
correctness. Noul has no separate confidence field. Proposed pilot thresholds
must be calibrated against known good/bad work, not interpreted as measured
error rates. [Confidence](https://docs.typesafe.ai/confidence)

## Startup behavior

Authenticated Jev behavior — **untested: no API key**. The actual missing-key
path was executed and reported its error to team-lead.

The proposed member directive runs `python3 scripts/jev_client.py --startup
--lead <appointed-lead>` before accepting work. Missing/malformed keys or a
failed authenticated synthetic probe produce the error envelope and an immediate
sanitized ATM error to the lead. The lead defaults to team-lead when no other
identity is appointed. Notification failure is surfaced for the coordinator to
retry. Rechecking startup after an environment update is sufficient; credentials
are never persisted. A passing startup proves authenticated operation only.

The helper also accepts an internal `--request` JSON file from the subagent,
validates Choice replies, and enforces small pilot request/retry bounds. It never
runs bead commands; only its explicit startup mode can notify the lead. The
subagent remains responsible for local lint and the unchanged outer Result.

## Open questions and pilot exit criteria

1. Will the user provide TypeSafe access and authorize sending selected source
   excerpts? Confirm account retention terms and the allowed repository scope.
2. Is the target reduced model cost, reduced wall time, or better detection?
   Include lint, evidence-preparation LLM cost and abstentions in the comparison.
3. What false-PASS rate and inconclusive rate are acceptable? Establish labelled
   fixtures: omitted criteria, plausible but wrong code, lint failures, already
   satisfied criteria, missing files, oversized diffs and misleading comments.
4. Can criteria/evidence mapping become deterministic for our bead format, or
   must the LLM wrapper remain? Evaluate missed candidates separately from Jev's
   classification quality.
5. Which pinned model and thresholds should be promoted after the pilot?
   Compare against `sc-sanity-llm` on the same commits and measure p50/p95 latency,
   actual usage charges, false PASS/FAIL and escalation rates before switching.

## Local verification

- `python3 -m unittest discover -s scripts/tests -p test_jev_client.py -v`:
  eight tests pass with mocked transport, including the startup lead notification.
- Payload and successful Result JSON examples compare equal to the role's
  examples; the error envelope retains exactly its four required fields.
- `bash scripts/ci/validate_docs_consistency.sh`: passed, including rustdoc gates.
- `git diff --check`: passed.
- Authenticated startup and Jev sanity accuracy: **untested: no API key**.

No user decision is required to review this draft. Production activation and
credentials remain open; the current checker continues to own the live gate.
