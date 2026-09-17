# B.P2 completeness fix checklist

- [ ] BP2-C01 implementation — build deterministic `.crate` archives with normalized manifests and archive provenance.
- [ ] BP2-C01 verification — inspect extracted archive contents, checksums, and dirty-source/rewrite rejection.
- [ ] BP2-C02 implementation — separate isolated baseline and candidate consumer legs.
- [ ] BP2-C02 verification — run both legs with exact provenance and locked re-resolution.
- [ ] BP2-C03 implementation — assert logging, querying, filtering, level state, reset, shutdown, and stale-owner behavior.
- [ ] BP2-C03 verification — retain assertion-bearing candidate output.
- [ ] BP2-C04 implementation — add three-platform CI evidence production and aggregate validation.
- [ ] BP2-C04 verification — prove the aggregate validator rejects incomplete or skipped platform evidence.
- [ ] BP2-C05 implementation — complete factual handoff, consumer documentation, and validation records.
- [ ] BP2-C05 verification — check recorded values against retained artifacts and commands.
- [ ] BP2-C06 implementation — make the authoritative `--version` command self-staging and update fix-layer metadata.
- [ ] BP2-C06 verification — run the documented command without `--stage`.
- [ ] BP2-C07 implementation — make query parity wait for deterministic writer completion.
- [ ] BP2-C07 verification — run the targeted parity test and retain its output.

## Second pass

- [ ] Revisit every BP2-C01 through BP2-C07 implementation and verification item against the pushed files and retained evidence.
