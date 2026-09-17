# Retained qualification failures

These are diagnostic failures, not passing artifact qualification evidence.

The macOS run executed the real desktop webviews with the e2b3304-era host staged unchanged except observation hooks, using the original packed client baseline and checkout dependencies for plumbing diagnosis. Both callers reached Tauri; plugin ACL lookup rejected calls before adapter entry. The report and raw build/runtime logs are retained independently of later successful runs.

The standalone-registry-drift report records the fail-closed artifact attempt at f248976. The staged source bundle initially promoted native dependency crates into workspace members, introducing their dev-dependency registry closure; the exact comparison correctly rejected it. The fix must preserve the reviewed standalone root closure and checksums.
