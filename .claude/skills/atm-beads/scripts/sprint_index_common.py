"""Shared process and canonical phase-plan helpers.

The persisted phase plan is JSONL, one tuple per nonblank line, in one of two formats (a file never mixes them):

- container: ``[sprint_name, wave, depends_on]``.  ``wave`` is an integer.  A ``depends_on`` entry is
  ``"d-12"`` (block on ``<prefix>d-12.chain.sanity``), ``["d-12", "qa"]`` (on ``.chain.qa``) or
  ``["d-12", "sprint"]`` (on the sprint bead ``<prefix>d-12``); the dependent's ``.chain.dev`` holds the edge.
- legacy: ``[sprint_name, sanity_bead_id, depends_on_sprint_names]``; the sprint bead is the dev bead.

Sprint and chain IDs are deliberately derived from the sprint name so a second copy cannot drift.

  sprint_index_common.py format <sprints.jsonl>                                   print container or legacy
  sprint_index_common.py check <sprints.jsonl> <beads.json> <root> plan|execution  graph problems of a container plan
"""
from __future__ import annotations
import json
import re
import subprocess
import sys
from pathlib import Path
from typing import Any

CONTAINER = "container"
LEGACY = "legacy"
CHAIN_STEPS = ("dev", "sanity", "qa")
# depends_on kind -> the prerequisite bead the dependent's .chain.dev blocks on
DEP_KINDS = ("sanity", "qa", "sprint")


def run_json(repo: Path, *args: str) -> Any:
    result = subprocess.run(list(args), cwd=repo, capture_output=True, text=True)
    if result.returncode:
        raise RuntimeError(f"{' '.join(args)} failed: {result.stderr.strip() or result.stdout.strip()}")
    try:
        return json.loads(result.stdout)
    except json.JSONDecodeError as exc:
        raise RuntimeError(f"{' '.join(args)} returned invalid JSON: {exc}") from exc


def phase_id(root: str, record: dict) -> str:
    phase = (record.get("metadata") or {}).get("phase")
    if not phase:
        match = re.search(r"-phase-([a-z0-9]+)$", root)
        phase = match.group(1) if match else None
    if not isinstance(phase, str) or not re.fullmatch(r"[a-z0-9]+", phase):
        raise RuntimeError(f"root bead {root} has no valid phase")
    return phase


def phase_path(repo: Path, phase: str) -> Path:
    if not re.fullmatch(r"[a-z0-9]+", phase):
        raise RuntimeError(f"invalid phase: {phase}")
    return repo / "docs" / "plans" / f"phase-{phase}" / "sprints.jsonl"


def _phase_from_path(path: Path) -> str:
    match = re.fullmatch(r"phase-([a-z0-9]+)", path.parent.name)
    if not match:
        raise RuntimeError("phase plan must live at docs/plans/phase-<phase>/sprints.jsonl")
    return match.group(1)


def root_prefix(root: str) -> str:
    """The id prefix of a phase root: ``obs-phase-d`` gives ``obs-`` (validate-plan's ``${root%%phase-*}``)."""
    return root.split("phase-", 1)[0]


def _dev_bead_id(sprint: str, prefix: str = "obs-") -> str:
    if not re.fullmatch(r"[a-z][a-z0-9]*(?:-[a-z0-9]+)*", sprint):
        raise RuntimeError(f"invalid sprint name {sprint!r}")
    return f"{prefix}{sprint}"


def chain_ids(sprint_bead: str) -> dict[str, str]:
    """The chain poured under a sprint container: the chain bead and its dev, sanity and qa steps."""
    chain = f"{sprint_bead}.chain"
    return {"chain": chain, **{step: f"{chain}.{step}" for step in CHAIN_STEPS}}


def dependency_target(sprint_bead: str, kind: str) -> str:
    """The bead a dependent's .chain.dev blocks on for a declared dependency of this kind."""
    return sprint_bead if kind == "sprint" else chain_ids(sprint_bead)[kind]


def _read_rows(path: Path) -> list[tuple[int, list]]:
    rows = []
    for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        try:
            value = json.loads(line)
        except json.JSONDecodeError as exc:
            raise RuntimeError(f"{path}:{number}: {exc}") from exc
        if not isinstance(value, list) or len(value) != 3:
            raise RuntimeError(f"{path}:{number}: each line must be [sprint_name, wave, depends_on] "
                               "(or the legacy [sprint_name, sanity_bead_id, depends_on])")
        rows.append((number, value))
    if not rows:
        raise RuntimeError(f"{path}: phase plan contains no sprints")
    return rows


def _row_format(value: list) -> str:
    second = value[1]
    if isinstance(second, int) and not isinstance(second, bool):
        return CONTAINER
    if isinstance(second, str):
        return LEGACY
    return "unknown"


def plan_format(path: Path) -> str:
    """``container`` or ``legacy``; a file that mixes both formats is an error."""
    formats = {}
    for number, value in _read_rows(path):
        fmt = _row_format(value)
        if fmt == "unknown":
            raise RuntimeError(f"{path}:{number}: the second field must be an integer wave or a legacy sanity bead id")
        formats.setdefault(fmt, number)
    if len(formats) > 1:
        raise RuntimeError(f"{path}: mixes container rows (line {formats[CONTAINER]}) with legacy rows "
                           f"(line {formats[LEGACY]}); a phase plan uses one format")
    return next(iter(formats))


def _parse_dependency(path: Path, number: int, entry: Any) -> tuple[str, str]:
    if isinstance(entry, str) and entry:
        return entry, "sanity"
    if (isinstance(entry, list) and len(entry) == 2 and isinstance(entry[0], str) and entry[0]
            and entry[1] in ("qa", "sprint")):
        return entry[0], entry[1]
    raise RuntimeError(f"{path}:{number}: a depends_on entry is \"<sprint>\", [\"<sprint>\", \"qa\"] "
                       f"or [\"<sprint>\", \"sprint\"], got {json.dumps(entry)}")


def parse_container_plan(path: Path, prefix: str = "obs-") -> list[dict]:
    """Rows of a container plan with the derived sprint and chain ids and each dependency's required edge.

    Each row: ``sprint``, ``wave``, ``sprint_bead``, ``chain`` (chain_ids), ``depends_on``
    (``[{sprint, kind, target}]``) and ``edges`` (``[(dependent .chain.dev, target, kind)]``).
    """
    rows: list[dict] = []
    for number, (sprint, wave, depends_on) in _read_rows(path):
        if not isinstance(sprint, str) or not sprint:
            raise RuntimeError(f"{path}:{number}: sprint_name must be a nonempty string")
        if not isinstance(wave, int) or isinstance(wave, bool) or wave < 0:
            raise RuntimeError(f"{path}:{number}: wave must be a nonnegative integer")
        if not isinstance(depends_on, list):
            raise RuntimeError(f"{path}:{number}: depends_on must be an array")
        deps = [_parse_dependency(path, number, entry) for entry in depends_on]
        names = [name for name, _ in deps]
        if sprint in names or len(set(names)) != len(names):
            raise RuntimeError(f"{path}:{number}: depends_on contains a duplicate or self dependency")
        if any(row["sprint"] == sprint for row in rows):
            raise RuntimeError(f"{path}:{number}: duplicate sprint {sprint}")
        sprint_bead = _dev_bead_id(sprint, prefix)
        rows.append({"sprint": sprint, "wave": wave, "sprint_bead": sprint_bead, "chain": chain_ids(sprint_bead),
                     "depends_on": [{"sprint": name, "kind": kind} for name, kind in deps]})
    by_sprint = {row["sprint"]: row for row in rows}
    for row in rows:
        unknown = sorted({dep["sprint"] for dep in row["depends_on"]} - set(by_sprint))
        if unknown:
            raise RuntimeError(f"{path}: {row['sprint']} depends_on unknown sprint(s): {', '.join(unknown)}")
        for dep in row["depends_on"]:
            dep["target"] = dependency_target(by_sprint[dep["sprint"]]["sprint_bead"], dep["kind"])
        row["edges"] = [(row["chain"]["dev"], dep["target"], dep["kind"]) for dep in row["depends_on"]]
    return rows


def parse_legacy_plan(path: Path, prefix: str = "obs-") -> list[dict]:
    rows: list[dict[str, object]] = []
    seen_sprints: set[str] = set()
    seen_sanity: set[str] = set()
    for number, (sprint, sanity, depends_on) in _read_rows(path):
        if not isinstance(sprint, str) or not sprint:
            raise RuntimeError(f"{path}:{number}: sprint_name must be a nonempty string")
        dev = _dev_bead_id(sprint, prefix)
        if not isinstance(sanity, str) or not sanity or sanity == dev:
            raise RuntimeError(f"{path}:{number}: sanity_bead_id must be a nonempty bead ID distinct from {dev}")
        if not isinstance(depends_on, list) or not all(isinstance(item, str) and item for item in depends_on):
            raise RuntimeError(f"{path}:{number}: depends_on must be an array of sprint names")
        if sprint in seen_sprints or sanity in seen_sanity:
            raise RuntimeError(f"{path}:{number}: duplicate sprint or sanity bead")
        if sprint in depends_on or len(set(depends_on)) != len(depends_on):
            raise RuntimeError(f"{path}:{number}: depends_on contains a duplicate or self dependency")
        seen_sprints.add(sprint)
        seen_sanity.add(sanity)
        rows.append({"sprint": sprint, "dev_bead_id": dev, "sanity_bead_id": sanity, "depends_on": depends_on})
    for row in rows:
        unknown = set(row["depends_on"]) - seen_sprints
        if unknown:
            raise RuntimeError(f"{path}: {row['sprint']} depends_on unknown sprint(s): {', '.join(sorted(unknown))}")
    return rows


def load_phase_plan(path: Path, root: str | None = None) -> dict:
    """Parse the compact JSONL plan and return the normalized runtime view.

    The normalized mapping is intentionally in-memory only.  Consumers retain
    their existing pair-oriented interface while all persisted graph authority
    remains the compact tuple file.  A container plan maps each sprint to its
    ``.chain.dev``/``.chain.sanity`` pair and carries ``format``, the sprint and
    qa ids, the wave and its ``qa``/``sprint`` dependency targets besides.
    """
    phase = _phase_from_path(path)
    # The root is supplied by the caller when known.  The filename supplies a
    # useful default for report discovery, but root naming itself is not a
    # duplicate field in the persisted tuple source.
    expected_root = root or f"obs-phase-{phase}"
    prefix = root_prefix(expected_root)
    if plan_format(path) == CONTAINER:
        normalized = []
        for row in parse_container_plan(path, prefix):
            chain = row["chain"]
            normalized.append({
                "dev_bead_id": chain["dev"], "sanity_bead_id": chain["sanity"],
                "depends_on_sanity_bead_ids": [d["target"] for d in row["depends_on"] if d["kind"] == "sanity"],
                "depends_on_bead_ids": [d["target"] for d in row["depends_on"] if d["kind"] != "sanity"],
                "sprint_bead_id": row["sprint_bead"], "qa_bead_id": chain["qa"], "wave": row["wave"]})
        result = {"root_bead_id": expected_root, "format": CONTAINER, "sprints": normalized}
        validate_index(result)
        return result
    rows = parse_legacy_plan(path, prefix)
    by_sprint = {str(row["sprint"]): row for row in rows}
    normalized = []
    for row in rows:
        prereq_sanity = [str(by_sprint[name]["sanity_bead_id"]) for name in row["depends_on"]]
        normalized.append({"dev_bead_id": row["dev_bead_id"], "sanity_bead_id": row["sanity_bead_id"],
                           "depends_on_sanity_bead_ids": prereq_sanity})
    result = {"root_bead_id": expected_root, "sprints": normalized}
    validate_index(result)
    return result


LEGACY_ROW_KEYS = frozenset({"dev_bead_id", "sanity_bead_id", "depends_on_sanity_bead_ids"})
CONTAINER_ROW_KEYS = LEGACY_ROW_KEYS | {"depends_on_bead_ids", "sprint_bead_id", "qa_bead_id", "wave"}


def index_bead_pairs(index: dict) -> dict[str, str]:
    """Validate explicit identity pairs without inventing IDs from naming conventions."""
    pairs: dict[str, str] = {}
    rows = index.get("sprints")
    if not isinstance(rows, list):
        raise RuntimeError("sprint index must contain a sprints array")
    allowed = CONTAINER_ROW_KEYS if index.get("format") == CONTAINER else LEGACY_ROW_KEYS
    for row in rows:
        if not isinstance(row, dict) or not {"dev_bead_id", "sanity_bead_id"} <= set(row) or set(row) - allowed:
            raise RuntimeError("each normalized sprint item must contain dev_bead_id, sanity_bead_id, and optional depends_on_sanity_bead_ids")
        dev, gate = row.get("dev_bead_id"), row.get("sanity_bead_id")
        if not isinstance(dev, str) or not dev:
            raise RuntimeError("dev_bead_id must be a nonempty bead ID")
        if not isinstance(gate, str) or not gate or gate == dev:
            raise RuntimeError(f"{dev}: missing or invalid sanity_bead_id; regenerate the sprint index")
        if dev in pairs or gate in pairs.values():
            raise RuntimeError(f"{dev}: duplicate dev or sanity bead in sprint index")
        deps = row.get("depends_on_sanity_bead_ids", [])
        if not isinstance(deps, list) or not all(isinstance(dep, str) and dep for dep in deps):
            raise RuntimeError(f"{dev}: depends_on_sanity_bead_ids must be a list of bead IDs")
        if len(set(deps)) != len(deps) or gate in deps:
            raise RuntimeError(f"{dev}: invalid duplicate or self sanity dependency")
        pairs[dev] = gate
    if set(pairs) & set(pairs.values()):
        raise RuntimeError("a sanity bead cannot also be a sprint dev bead")
    known_sanity = set(pairs.values())
    # container plans also block on a prerequisite's qa step or sprint bead
    known_other = {row.get(key) for row in rows for key in ("sprint_bead_id", "qa_bead_id")} - {None}
    for row in rows:
        for dep in row.get("depends_on_sanity_bead_ids", []):
            if dep not in known_sanity:
                raise RuntimeError(f"{row['dev_bead_id']}: dependency {dep} is not a listed sprint sanity bead")
        for dep in row.get("depends_on_bead_ids", []):
            if dep not in known_other:
                raise RuntimeError(f"{row['dev_bead_id']}: dependency {dep} is not a listed sprint or qa bead")
    return pairs


REQUIRED_KEYS = frozenset({"root_bead_id", "sprints"})
# Phase facts that are not in beads (user ruling 2026-09-26); membership rows stay exactly as before.
# `format` marks the in-memory view of a container plan.
OPTIONAL_KEYS = frozenset({"integration_branch", "review_artifacts", "policy", "format"})
POLICY_KEYS = frozenset({"human_gates", "waivers"})
WAIVABLE_CHECKS = frozenset({"reopened_after_pass", "started_before_blocker", "pass_without_qa"})


def validate_index(index: Any) -> None:
    """Enforce the document contract: membership rows plus declared phase facts, nothing else."""
    if not isinstance(index, dict) or not REQUIRED_KEYS <= set(index):
        raise RuntimeError("phase index must contain root_bead_id and sprints")
    unknown = set(index) - REQUIRED_KEYS - OPTIONAL_KEYS
    if unknown:
        raise RuntimeError("phase index may contain only root_bead_id and sprints plus the declared phase facts "
                           f"({', '.join(sorted(OPTIONAL_KEYS))}); undeclared keys: {', '.join(sorted(unknown))}")
    fmt = index.get("format")
    if fmt is not None and fmt != CONTAINER:
        raise RuntimeError(f"format must be {CONTAINER} when present")
    branch = index.get("integration_branch")
    if branch is not None and (not isinstance(branch, str) or not branch):
        raise RuntimeError("integration_branch must be a nonempty branch name")
    artifacts = index.get("review_artifacts")
    if artifacts is not None and (not isinstance(artifacts, list) or not all(isinstance(a, str) and a for a in artifacts)):
        raise RuntimeError("review_artifacts must be a list of repository paths")
    policy = index.get("policy")
    if policy is not None:
        if not isinstance(policy, dict) or set(policy) - POLICY_KEYS:
            raise RuntimeError(f"policy may only contain {', '.join(sorted(POLICY_KEYS))}")
        gates = policy.get("human_gates")
        if gates is not None and (not isinstance(gates, list) or not all(isinstance(g, str) and g for g in gates)):
            raise RuntimeError("policy.human_gates must be a list of gate bead IDs")
        waivers = policy.get("waivers")
        if waivers is not None:
            if not isinstance(waivers, list):
                raise RuntimeError("policy.waivers must be a list of {bead, check, reason} objects")
            for w in waivers:
                if not isinstance(w, dict) or set(w) != {"bead", "check", "reason"} or not all(isinstance(w[k], str) and w[k] for k in w):
                    raise RuntimeError("each policy.waivers entry has exactly bead, check and reason, all nonempty strings")
                if w["check"] not in WAIVABLE_CHECKS:
                    raise RuntimeError(f"policy.waivers check must be one of {', '.join(sorted(WAIVABLE_CHECKS))}")
    root = index["root_bead_id"]
    if not isinstance(root, str) or not root:
        raise RuntimeError("root_bead_id must be a nonempty bead ID")
    pairs = index_bead_pairs(index)
    if root in pairs or root in pairs.values():
        raise RuntimeError("phase root cannot also be a dev or sanity bead")


# --- container plan graph checks (validate-plan) ---

def bead_parent(bead: dict) -> str | None:
    """``parent`` as bd prints it, else the parent-child dependency (rendered import JSONL)."""
    if bead.get("parent"):
        return bead["parent"]
    for dep in bead.get("dependencies") or []:
        if (dep.get("type") or dep.get("dependency_type")) == "parent-child":
            return dep.get("depends_on_id") or dep.get("id")
    return None


def bead_blocks(bead: dict) -> list[str]:
    """Targets of a bead's blocks edges, in bd list/import shape or bd show shape."""
    return [dep.get("depends_on_id") or dep.get("id") for dep in bead.get("dependencies") or []
            if (dep.get("type") or dep.get("dependency_type")) == "blocks"]


def container_problems(rows: list[dict], beads: list[dict], root: str, mode: str) -> list[str]:
    """One "<bead>: <problem>" line per graph problem of a container plan.

    plan: every sprint bead exists, its metadata.sprint and metadata.wave match its tuple, no prerequisite is in a
    later wave, and every open stage:sprint child of the root is a tuple.  execution also: each sprint has one
    complete chain and each dev step's cross-sprint blocks edges are exactly the declared ones.
    """
    by_id = {b.get("id"): b for b in beads}
    by_sprint = {row["sprint"]: row for row in rows}
    listed = {row["sprint_bead"] for row in rows}
    out: list[str] = []
    for row in rows:
        sid, bead = row["sprint_bead"], by_id.get(row["sprint_bead"])
        if bead is None:
            out.append(f"{sid}: sprint {row['sprint']} is in sprints.jsonl but not in beads")
            continue
        meta = bead.get("metadata") or {}
        if meta.get("sprint") != row["sprint"]:
            out.append(f"{sid}: metadata.sprint is {json.dumps(meta.get('sprint'))}, not {row['sprint']}")
        wave = meta.get("wave")
        if isinstance(wave, int) and not isinstance(wave, bool) and wave != row["wave"]:
            out.append(f"{sid}: metadata.wave is {wave}, sprints.jsonl wave is {row['wave']}")
        for dep in row["depends_on"]:
            if by_sprint[dep["sprint"]]["wave"] > row["wave"]:
                out.append(f"{sid}: prerequisite {dep['sprint']} is in wave {by_sprint[dep['sprint']]['wave']}, "
                           f"after this sprint's wave {row['wave']}")
    for bead in beads:
        if (bead.get("id") not in listed and bead.get("status") != "closed" and bead_parent(bead) == root
                and "stage:sprint" in (bead.get("labels") or [])):
            out.append(f"{bead.get('id')}: open sprint under {root} is not in sprints.jsonl")
    if mode != "execution":
        return out
    # every id a sprint owns, so a dev step's cross-sprint blockers can be told from gates and other beads
    owner = {}
    for row in rows:
        for bid in (row["sprint_bead"], *row["chain"].values()):
            owner[bid] = row["sprint"]
    for row in rows:
        sid, chain = row["sprint_bead"], row["chain"]
        if sid not in by_id:
            continue
        missing = [chain[k] for k in ("chain", *CHAIN_STEPS) if chain[k] not in by_id]
        if missing:
            out.append(f"{sid}: incomplete chain, missing {', '.join(missing)}; pour it before dispatch")
            continue
        # a step's own parent is checked by the ChainStep model (bead_schema.py)
        if bead_parent(by_id[chain["chain"]]) != sid:
            out.append(f"{chain['chain']}: parent is {json.dumps(bead_parent(by_id[chain['chain']]))}, not {sid}")
        for step, needs in (("sanity", "dev"), ("qa", "sanity")):
            if chain[needs] not in bead_blocks(by_id[chain[step]]):
                out.append(f"{chain[step]}: missing blocks edge to {chain[needs]}")
        dev = by_id[chain["dev"]]
        actual = {t for t in bead_blocks(dev) if t in owner and owner[t] != row["sprint"]}
        declared = {dep["target"] for dep in row["depends_on"]}
        reported: set[str] = set()
        for dep in row["depends_on"]:
            if dep["target"] in actual:
                continue
            theirs = sorted(t for t in actual - declared if owner[t] == dep["sprint"])
            reported.update(theirs)
            if theirs:
                out.append(f"{chain['dev']}: blocks on {', '.join(theirs)} instead of {dep['target']} "
                           f"(sprints.jsonl: {row['sprint']} depends on {dep['sprint']} ({dep['kind']}); "
                           "a declared edge is never weakened or changed)")
            else:
                out.append(f"{chain['dev']}: missing blocks edge to {dep['target']} "
                           f"(sprints.jsonl: {row['sprint']} depends on {dep['sprint']} ({dep['kind']}))")
        for target in sorted(actual - declared - reported):
            out.append(f"{chain['dev']}: blocks edge to {target} is not declared in sprints.jsonl")
    return out


def main(argv: list[str]) -> int:
    try:
        if len(argv) == 2 and argv[0] == "format":
            print(plan_format(Path(argv[1])))
            return 0
        if len(argv) == 5 and argv[0] == "check" and argv[4] in ("plan", "execution"):
            rows = parse_container_plan(Path(argv[1]), root_prefix(argv[3]))
            found = container_problems(rows, json.loads(Path(argv[2]).read_text()), argv[3], argv[4])
            print("\n".join(found)) if found else None
            return 5 if found else 0
    except RuntimeError as exc:
        print(exc)
        return 5
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as exc:
        print(f"sprint_index_common: {exc}", file=sys.stderr)
        raise SystemExit(2)
