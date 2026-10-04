# Pinned viewer fixture

`release.json` currently pins only the verified `darwin_arm64` viewer release.
On Windows, `download_pinned_release.py` deliberately exits before choosing an
output path or downloading anything: no Windows artifact URL or SHA-256 has
been verified for this pin. The download test exercises that real main-path
refusal, while `_output_path` separately guards the eventual Windows `.exe`
destination. Add a verified Windows artifact pin before enabling Windows
downloads through `main`.
