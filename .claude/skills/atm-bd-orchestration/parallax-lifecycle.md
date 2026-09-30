# Parallax Lifecycle

The lead spins a parallax agent up for each wave and down after its drain,
in the herdr workspace of the team. At most two run at once, so the
`parallax` tab never holds more than two panes, and every agent that stops
leaves the roster, its pane and its inbox.

| Name | Value |
| --- | --- |
| `NAME` | `<team prefix>-parallax-<phase><wave>`, e.g. `obs-parallax-d3`; must match `^[a-z][a-z0-9_-]{0,31}$` (herdr agent names are host-global) |
| `TEAM`, `WS` | the ATM team and its herdr workspace id (`sc-obs`, `wP`) |
| `WT` | the wave's worktree ("Assigning A Wave", step 2) |
| `MODEL` | the model the lead picked for the wave |

Use the raw commands below. `hmux` spawn mode is not used: it registers the
wrong agent type, drops the model and harness, and takes no directive.
Never `herdr agent start`.

## Spin-Up

1. Check that `NAME` is new. `atm members --team $TEAM --json | jq -e
   '.members[] | select(.name=="'$NAME'")'` succeeds for a live member: stop,
   it is running. A member whose pane is dead is recovered first (Recovery).
2. Find the tab: `herdr tab list --workspace $WS`, label `parallax`. If it is
   absent, `herdr tab create --workspace $WS --label parallax` and use its
   `root_pane_id` as the pane. If it holds two live panes, stop and report:
   the older agent has not finished its drain.
3. Otherwise `herdr pane split --pane <last pane of the tab> --direction down`.
4. `herdr pane rename <PANE> $NAME`.
5. Register it:

   ```bash
   atm teams add-member $TEAM $NAME --agent-type <codex|claude-code> --backend herdr \
     --model $MODEL --home-dir $WT
   ATM_TEAM=$TEAM ATM_IDENTITY=team-lead atm teams update-member $TEAM $NAME \
     --harness <codex-cli|claude-code> --model $MODEL
   ```

   Never pass `--session`. `add-member` always records harness
   `claude-code`; the `update-member` call corrects it.
6. Launch it with the directive as its first prompt:

   ```bash
   # codex
   herdr pane run <PANE> "cd $WT; export ATM_IDENTITY=$NAME ATM_TEAM=$TEAM BEADS_ACTOR=$NAME; codex -c features.hooks=true --yolo --model $MODEL 'read .claude/agents/parallax.md for your directive, then await your first task'"
   # claude-code
   herdr pane run <PANE> "cd $WT; export ATM_IDENTITY=$NAME ATM_TEAM=$TEAM BEADS_ACTOR=$NAME; claude --model $MODEL --dangerously-skip-permissions 'read @file:.claude/agents/parallax.md for your directive, then await your first task'"
   ```

7. `herdr agent rename <PANE> $NAME`.
8. Verify: `herdr agent list` shows `NAME` idle or working, and `atm members
   --team $TEAM` shows it active or idle, availability `fresh`, `live_cwd` =
   `WT`, pane = `PANE`. Then assign the wave.

No `.atm.toml` or `registry.yaml` entry is added.

## Spin-Down

After the agent's `DRAINED` report and handover, once the lead has checked
them ("Overseeing A Wave"):

1. `herdr agent prompt $NAME "You are done for this wave. Send your final status to the lead, then exit."`
2. `herdr agent wait $NAME --until done --timeout 300000`; no process for
   `NAME` remains.
3. `ATM_TEAM=$TEAM ATM_IDENTITY=team-lead atm teams remove-member $TEAM $NAME`
4. `herdr pane close <PANE>`. If the `parallax` tab now holds no panes,
   `herdr tab close <tab id>`.
5. Remove what `remove-member` leaves behind:

   ```bash
   rm -f ~/.atm/.claude/teams/$TEAM/inboxes/$NAME.json
   rm -f $WT/.sc/sessions/{codex,claude}/*$NAME*
   ```

   If `herdr agent list` still shows `NAME`, `herdr agent rename $NAME --clear`.

## Recovery

A member whose pane is dead (not in `herdr agent list`): close its pane if
it still shows, run Spin-Down steps 3 to 5, then Spin-Up from step 2. Its
open tasks and beads stay as they are; the new agent's wave assignment
lists them.
