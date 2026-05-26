# API Approval Artifacts

This directory is the machine-checkable approval surface for intentional public
API changes introduced after the currently shipped baseline.

## Required Shape

- one file per intentional API change
- file name format: `<change-id>.md`
- required headings in every approval file:
  - `## Scope`
  - `## Approval`
  - `## Affected Artifacts`

## Phase-A Usage

Phase A uses this directory as the approval-artifact target referenced by
[`docs/phase-A/sprint-A2.md`](../phase-A/sprint-A2.md).

The directory may remain empty until the first intentional public API change is
proposed, but the directory and its required artifact format must exist before
`validate_public_api_docs.sh` is implemented.
