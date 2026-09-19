# C.1 legacy publish asset inventory

This is the C.1 disposition inventory for the validated upstream kit at
`61568d813550ac7faab0fd9c887c731d975d1264`. The kit is now installed on the
C.1 layer; `release/sc-publish-pin.toml` records the immutable source revision.
The remaining acceptance evidence is the repeat no-drift parity and consumer
compatibility qualification described by the sprint checklist.

| Consumer path | Current state | Planned disposition |
| --- | --- | --- |
| `.claude/agents/publisher.md` | Repository-owned legacy publisher prompt | Same-path byte-for-byte overwrite from the approved kit |
| `.github/workflows/release.yml` | Legacy release workflow | Same-path overwrite from the approved kit |
| `.github/workflows/release-preflight.yml` | Legacy preflight workflow | Same-path overwrite from the approved kit |
| `scripts/release_artifacts.py` | Legacy manifest helper | Delete after installed `.github/scripts/release_artifacts.py` validates the rendered manifest |
| `scripts/ci/validate_publish_order.sh` | Legacy order check | Delete after shared `validate-publish-order` passes |
| `.claude/agents/{publisher-channel-protocol,*-publisher}.md` | Not present or partially present | Add from the approved kit |
| `.claude/skills/publishing/**` | Not present as the shared skill | Add from the approved kit |
| `.github/workflows/{crates,homebrew,npm,pypi,scoop,winget}-publish.yml` | Not present as the shared channel set | Add from the approved kit |
| `release/RELEASE-NOTES-TEMPLATE.md` | Repository-owned release notes template | Retain unchanged |
| `release/release-inventory.json` | Repository-owned inventory | Retain and reconcile to the rendered ten-crate/five-wheel/npm inventory |
| `docs/release-readiness-checklist.md` | Independent qualification checklist | Retain unchanged |

The approved upstream kit also installs `.github/scripts/release_manifest.py`
and `release_python.py`; these are distinct shared replacements and are not
present under the legacy `scripts/` paths. All old references must be updated
before either legacy script is removed.
