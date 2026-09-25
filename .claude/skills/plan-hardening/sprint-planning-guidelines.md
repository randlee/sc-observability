# Sprint Planning Guidelines

Use these rules when hardening sprint plans.

## Core Rules

- The sprint plan is authoritative.
- Downstream prompts may carry structured projections of sprint-plan data, but
  they must not replace or narrow the sprint plan.
- If QA cannot review directly from the sprint doc, the sprint doc is not
  hardened.

## Split Early

Split a sprint immediately when any of these are true:

- there is credible doubt that every committed deliverable can land at a
  production-ready level in the same sprint
- the sprint mixes too many closure types
- the sprint touches too many modules, boundaries, or runtime paths for clear
  ownership
- acceptance criteria would allow one deliverable to slip while the sprint
  still claims success
- the same deliverable is being planned more than once across multiple sprints

Do not preserve an overloaded sprint just to keep the sprint count low.

## Sprint Doc Shape

Each sprint doc should have one authoritative list for:

- deliverables
- acceptance criteria
- paths to delete, when applicable
- required validation

Do not restate the same checklist item in multiple sections with different
wording.

## Production-Ready Expectation

Every listed deliverable must be expected to land at a production-ready level
for the scope that sprint claims.

Do not allow:

- shape-only completion
- test-only completion
- boundary-only completion when runtime behavior is still open
- silent carry-forward of a committed deliverable

If a sprint intentionally does not close something, state that explicitly under
non-closure or out-of-scope sections.

## Code Samples

Important traits, enums, protocol types, interfaces, and boundary contracts
must have explicit code samples or signatures in the sprint doc when prose
alone would leave implementation choices open.

## Recommended Agent / Model

Optional `recommended_agent`/`recommended_model` select from the current
developer pool: a fast agent for bounded or documentation work; a
deep-reasoning agent for algorithmic, architectural, or performance work.
They are advice, not an assignment.

## Dependency Relations

List each related sprint as `must_follow` or `parallel_safe` with a rationale.
`must_follow` merge-forward trigger: parent development is pushed, not QA;
merge parent → child before every dev/fix round. PR-completion trigger: parent
PR merges first. `parallel_safe` requires
non-intersecting modules/crates, public contracts, artifacts, and ownership.
Prefer parallel-safe splits where credible. Plan-scope-reviewer verifies this.

A `must_follow` edge needs concrete coupling: the same files/crates/public
types, or the child consumes the parent's code. Shared release/version
baseline alone is not coupling; handle it with a final integration step.
Parallel tracks run as separate gh-stack stacks with named branches,
worktrees, and assigned agents.

## Process Artifacts

A plan may require a process artifact (manifest, inventory, ledger, receipt,
matrix, docs-consistency check, new CI gate) only if the sprint doc names:
its consumer, the capability it gates, the observed defect it prevents (not
speculative), and when it is retired. Otherwise leave it out. Prefer what
already enforces the property: the compiler, existing tests, existing CI,
git history. A plan-writing rule (e.g. single ownership of a contract) stays
a plan rule; it does not become a product CI gate.

Frontmatter `status` describes the sprint, not the plan: an unimplemented
sprint is `planned`, never `complete`.

## QA Consumption

Sprint docs must be short and structured enough that:

- `req-qa` can enumerate deliverables and acceptance criteria directly
- `arch-qa` can identify structural gate artifacts directly
- `quality-mgr` can route QA without copying scope by hand

If that is not true, shorten or tighten the sprint doc instead of adding more
prompt ceremony.

## Finding Classification

Classify each finding as either structural or wording before assigning
severity.

Structural findings:
- missing acceptance or validation gate
- incorrect command, test name, or grep gate
- uncovered call site, file, module, or runtime path
- missing type, trait, function, boundary contract, or ADR
- false-closure wording that hides still-open runtime or boundary work

Structural findings always remain in the main `findings` array and must be
rated `Blocking` or `Important` when they affect implementability or closure.

A "missing gate" finding is structural only when a deliverable's behavior
would otherwise go unverified. A finding whose only remedy is a new process
artifact must pass the Process Artifacts rule above; otherwise it is debt
notes, not a finding. Over-specification (unjustified artifacts, restated
contracts, redundant inventories) is itself a valid finding.

Wording findings:
- prose ambiguity that does not change scope or closure meaning
- formatting cleanup
- non-normative wording polish

Wording findings belong in `minor_wording` and do not fail the round unless
the reviewer marks them `affects_ac: true`.
