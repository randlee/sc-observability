# Prerequisites

The skill needs these installed:

- `bd` 1.3.0 or newer
- `atm`
- the standalone `sc-compose` CLI, even though `atm` embeds it
- `jq`
- `gh` with the `gh-stack` extension
- the `sc-gh-stack` and `sc-git-worktree` skills (synaptic-canvas plugin)

Check them on setup, or when a command is not found:

```bash
for c in bd atm sc-compose jq gh; do command -v "$c" >/dev/null && echo "ok $c" || echo "MISSING $c"; done
bd version    # 1.3.0 or newer
gh stack --version   # the gh-stack extension
```

If anything is missing or too old, read
[`../atm-beads/references/installation-and-troubleshooting.md`](../atm-beads/references/installation-and-troubleshooting.md).
