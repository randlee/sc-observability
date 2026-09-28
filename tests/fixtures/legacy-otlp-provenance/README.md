# Immutable legacy OTLP provenance source

`source.pack` contains the original commit object for
`7b39f4e7f72b6845edec4eab4cd671611661445f`, the tree objects needed to traverse
the twelve paths in `docs/plans/phase-d/legacy-otlp-provenance.json`, and those
twelve exact source blobs. No parent history or unrelated file contents are
included. `objects.json` lists every object, its type/path, and the pack SHA-256.
The original commit is available in the recovery repository but is not fetchable
from GitHub. This fixture makes clean and shallow checkouts reproducible.

CI runs `python3 scripts/ci/provision_legacy_otlp_provenance.py` before Rust tests.
It checks the fixture checksum before importing Git objects, then verifies their
types and paths against the original commit. It does not check out or execute
historical source. Existing provenance tests still verify all original blob IDs
and SHA-256 values and reject tampering with each manifest entry.

To reproduce the pack in a repository containing the original objects, run from
the repository root (Git compression changes can change pack bytes, not objects):

```bash
python3 - <<'PY'
import hashlib, json, subprocess
from pathlib import Path
fixture = Path('tests/fixtures/legacy-otlp-provenance')
inventory = json.loads((fixture / 'objects.json').read_text())
object_ids = ''.join(oid + '\n' for oid in sorted(inventory['objects']))
packed = subprocess.check_output(
    ['git', 'pack-objects', '--stdout', '--no-reuse-delta',
     '--no-reuse-object', '--compression=9'], input=object_ids.encode())
assert hashlib.sha256(packed).hexdigest() == inventory['pack_sha256']
assert packed == (fixture / 'source.pack').read_bytes()
PY
```

The sparse historical tree is intentionally not a checkout fixture. Only the
manifest paths are materialized as blobs; tests inspect them with `git show`.
