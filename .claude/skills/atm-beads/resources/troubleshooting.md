# Troubleshooting

## Identity

`ATM_IDENTITY` and `BEADS_ACTOR` are set by the session launcher. Check them
when a bead is claimed or closed by the wrong actor, or `bd ready -a <agent>`
misses work that is assigned to that agent:

```bash
echo "ATM_IDENTITY=[$ATM_IDENTITY] BEADS_ACTOR=[$BEADS_ACTOR] ATM_TEAM=[$ATM_TEAM]"
```

| Symptom | Cause | Fix |
| --- | --- | --- |
| `BEADS_ACTOR=[]` | the pane was started without it | restart the pane from the launcher; do not export it by hand |
| `BEADS_ACTOR` differs from `ATM_IDENTITY` | stale or hand-set value | restart the pane; stop `bd` writes until they match |
| value is an alias (`obs-lead`) or model class (`terra`) | wrong launcher entry | fix the pane entry to the bare identity, then restart |
| `bd ready -a <agent>` is empty but work is assigned | bead assignee differs from the identity | `bd update <bead> -a <identity>` |

Until the two match, do not claim, close or create beads: the actor recorded
on the bead would be wrong.
