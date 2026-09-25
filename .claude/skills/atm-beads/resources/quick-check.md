# Quick-Check

The assignment for a quick-check bead. When and how it runs is in
[`orchestrating.md`](orchestrating.md) ("Quick-Check").

## Recipient

A dedicated quick-check agent (not a dev or fix agent, which would make
quick-checks wait behind their work). For each quick-check it starts an
independent background agent, concurrently up to its harness's limit, and
never serializes unrelated checks on purpose. The background agents run luna
today.

The quick-check bead's `assignee` is that agent's `ATM_IDENTITY` (the
examples call it `quick-check`). It must be an ATM member of the team:
`validate-plan` rejects any other name. If the team has no such member,
lead adds one before the plan is imported (the `team-lead` skill, Step 3).

Candidates to replace luna later: Haiku 5.5 once released (Haiku 4.5 is too
weak for this), and typesafe.ai (<https://typesafe.ai/>), to be evaluated.

## Message

The quick-check agent sends this message to a background agent with the dev
bead piped into its prompt, as in [`orchestrating.md`](orchestrating.md)
("Passing A Bead To A Background Agent"). The quick-check bead's description
carries the same message, so the instruction can be taken from it:
`bd show <quick-check-bead> --json | jq -r '.[0].description'`.


Verify that the work is done:

- the agent did not skip anything;
- there are no obvious errors;
- lint passes.

This is not QA.
