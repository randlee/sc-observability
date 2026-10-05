"""One multiplicity-aware difference formatter for public API row projections."""
from collections import Counter


def row_differences(reference: list[str], candidate: list[str]) -> list[str]:
    """Rows whose multiplicity differs; `-` only in reference, `+` only in candidate."""
    before, after = Counter(reference), Counter(candidate)
    lines = []
    for row in sorted(set(before) | set(after)):
        delta = after[row] - before[row]
        if delta < 0:
            lines.append(f'-{abs(delta)}x {row}' if abs(delta) > 1 else f'- {row}')
        elif delta > 0:
            lines.append(f'+{delta}x {row}' if delta > 1 else f'+ {row}')
    return lines
