# Quick-Check

The assignment for a quick-check bead. When and how it runs is in
[`orchestrating.md`](orchestrating.md) ("Quick-Check").

## Recipient

A dedicated quick-check agent (not a dev or fix agent, which would make
quick-checks wait behind their work). It starts one background agent per
quick-check, so checks run in parallel and never queue. The background agents
run luna today.

Candidates to replace luna later: Haiku 5.5 once released (Haiku 4.5 is too
weak for this), and typesafe.ai (<https://typesafe.ai/>), to be evaluated.

## Message

Verify that the work is done:

- the agent did not skip anything;
- there are no obvious errors;
- lint passes.

This is not QA.
