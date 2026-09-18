# API Approval Artifacts

This directory is the machine-checkable approval surface for intentional public
API changes introduced after the currently shipped baseline.

## Required Shape

- one file per intentional API change
- Markdown records use the file name format `<change-id>.md`; B.2 and later
  machine-checkable API governance records use `<change-id>.json` and the
  schema described below.
- required headings in every Markdown approval record:
  - `## Scope`
  - `## Approval`
  - `## Affected Artifacts`

## Phase-A Usage

Phase A uses this directory as the approval-artifact target referenced by
[`docs/plans/phase-a/sprint-A2.md`](../plans/phase-a/sprint-A2.md).

The directory may remain empty until the first intentional public API change is
proposed, but the directory and its required artifact format must exist before
`validate_public_api_docs.sh` is implemented.

Validation behavior:

- `validate_public_api_docs.sh` passes when this `README.md` is the only file
  in the directory and no public API diff is detected, or when each detected
  diff has a matching scoped approval record
- `validate_public_api_docs.sh` fails when a public API diff is detected and no
  non-`README.md` approval artifact exists
- `validate_public_api_docs.sh` fails when any approval artifact is missing one
  or more required headings

## B.2 crate-scoped review records

The current qualification uses JSON records with schema_version 1, the exact
candidate_version, and a crates map keyed by each reviewed package name. Each
entry needs status `approved`, an actual reviewer, review evidence, `public-api`
scope and the exact `api_sha256` from the successful per-crate diff report. A
pending, unrelated, different-version or different-API record does not pass.
These records govern API review only: they cannot waive tool execution or
semver failures and do not grant live publication or runtime owner acceptance.
