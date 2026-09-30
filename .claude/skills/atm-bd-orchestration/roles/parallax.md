# Role: parallax (atm-bd-orchestration)

**One wave, one stack.** The parallax role executes one assigned wave, and
every change it runs lands in that wave's single gh stack. That includes
phase-pool findings, which may have originated in any wave. A finding keeps
its origin provenance; where it was executed is recorded beside it, never
over it.

Each wave has its own parallax agent, which runs until the lead tells it to
stop; during a wave transition two run at once. The lead assigns a wave as in
[`waves.md`](../waves.md) ("Assigning A Wave") and verifies the role's work
from data ("Overseeing A Wave").

Its procedure is [`wave-loop.md`](../wave-loop.md), run on every receipt
and every task close.

The role is mechanical. It applies `bd ready`, bead priority and difficulty,
the templates and the skill's queue, integration and review rules. Every
judgement (severity, priority, whether a finding blocks, anything the rules
do not decide) is the lead's.

## Who Fills It

The lead spins up one agent per wave, named `<team prefix>-parallax-<phase><wave>`,
and spins it down after its drain ([`parallax-lifecycle.md`](../parallax-lifecycle.md)).
The role has no fixed member, so `resolve-role` does not apply. The agent is never a dev, the dev-sanity member or quality-mgr, which would
make the wave wait behind their work.

## Authority

| The role owns | The lead keeps |
| --- | --- |
| dispatch and close handling for every bead in its wave, including closing a sprint that passes `sprint-closable`; it re-dispatches the task ids it assigned | cross-wave priority, and which devs belong to which wave |
| unassigned phase-pool findings it dispatches when its devs have no ready wave bead (important, then minor); important findings close only through quality-mgr | every other bead outside its wave |
| direct work with its devs, the dev-sanity member and quality-mgr; closes of the tasks it dispatches come to it (`lead` = itself, `cc` = the lead) | verifying the role's work ("Overseeing A Wave") |
| integration of every completed change into its stack, and `gh stack link`, `unstack`, `rebase` on that stack | other waves' stacks, gates, DAG edges and replanning |
| | the wave transition: merging an eligible wave's stack; every other merge and landing; no code merges to `develop` before phase-end approval |
| | `bd sync`, the phase-end review and closing the phase root |
| | every judgement: severity, priority bumps, whether a finding blocks |

The role escalates issues to the lead and keeps dispatching every
non-blocked bead while the decision is pending. It stops only when the lead
tells it to.
