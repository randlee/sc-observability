#!/usr/bin/env python3
# DO NOT EDIT WITHOUT EXPLICIT USER APPROVAL.
# A bead that fails these models is fixed in the bead, never by changing the models. Report the problem instead.
"""Pydantic models of the schema-type beads. The published JSON Schemas (atm-beads/schemas/) are exported from them.

  bead_schema.py validate <beads.json> <sprints.jsonl> <id prefix>   one "<bead>: <field>: <problem>" line per problem
  bead_schema.py export <dir>                                          write <dir>/<name>.schema.json for every model

A container sprints.jsonl validates each sprint container against SprintBead and each poured chain step against
ChainStep; a legacy one validates each sprint dev bead against LegacySprintBead and its sanity bead against SanityBead.

Exit 0 valid, 5 problems, 2 could not run.
"""

from __future__ import annotations

import json
import sys
from pathlib import Path
from typing import Annotated, Literal

from pydantic import (AfterValidator, AliasChoices, BaseModel, ConfigDict, Field, ValidationError, ValidationInfo,
                      model_validator)

sys.path.insert(0, str(Path(__file__).resolve().parent))
from sprint_index_common import CONTAINER, chain_ids, parse_container_plan, parse_legacy_plan, plan_format  # noqa: E402

# the description holds a numbered '## Deliverables' list starting at 1
DELIVERABLES = r"(?m)^## Deliverables\s*\n(?:\s*\n)*\s*1[.)]\s+\S"
NonEmpty = Annotated[str, Field(min_length=1)]
Wave = Annotated[int, Field(strict=True, ge=0)]
Difficulty = Literal["hard", "normal", "fast"]
# chain step -> its stage label, as sprint-chain.formula.toml.j2 bakes it
STEP_STAGE = {"dev": "stage:dev", "sanity": "stage:dev-sanity", "qa": "stage:qa"}


def ids_or_none(v: list[str]) -> list[str]:
    if not v:
        raise ValueError('list the ids, or exactly ["NONE"]')
    if "NONE" in v and len(v) > 1:
        raise ValueError("mixes NONE with ids")
    return v


IdList = Annotated[list[NonEmpty], AfterValidator(ids_or_none)]


class Dependency(BaseModel):
    model_config = ConfigDict(extra="allow")
    type: str = Field(validation_alias=AliasChoices("type", "dependency_type"))
    depends_on_id: str = Field(validation_alias=AliasChoices("depends_on_id", "id"))


class Node(BaseModel):
    """Any bead: its id and dependency edges."""
    model_config = ConfigDict(extra="allow")
    id: NonEmpty
    dependencies: list[Dependency] = []

    def blocks(self) -> list[str]:
        return [d.depends_on_id for d in self.dependencies if d.type == "blocks"]

    def parent_id(self) -> str | None:
        """``parent`` as bd prints it, else the parent-child dependency (rendered import JSONL)."""
        parent = (self.model_extra or {}).get("parent")
        if parent:
            return parent
        return next((d.depends_on_id for d in self.dependencies if d.type == "parent-child"), None)


class Bead(Node):
    """A dispatched bead: it has an assignee."""
    assignee: NonEmpty


def wave_label(labels: list[str], wave: int) -> str | None:
    """The problem with a bead's wave:<n> labels against its metadata.wave, or None."""
    waves = [label for label in labels if label.startswith("wave:")]
    if waves != [f"wave:{wave}"]:
        return f"labels {waves} must be exactly [\"wave:{wave}\"] (metadata.wave)"
    return None


class LegacySprintMetadata(BaseModel):
    model_config = ConfigDict(extra="allow")
    requirements: IdList
    adrs: IdList
    worktree: NonEmpty
    branch: NonEmpty
    pr_target: NonEmpty
    difficulty: Difficulty


class LegacySprintBead(Bead):
    """A legacy stage:sprint dev bead (sprints.jsonl rows [sprint, sanity bead, deps])."""
    description: Annotated[str, Field(pattern=DELIVERABLES)]
    acceptance_criteria: NonEmpty
    metadata: LegacySprintMetadata


class SprintMetadata(BaseModel):
    model_config = ConfigDict(extra="allow")
    phase: NonEmpty
    sprint: NonEmpty
    wave: Wave
    stack: NonEmpty
    layer: Annotated[int, Field(strict=True, ge=1)]
    pr_target: NonEmpty
    difficulty: Difficulty
    requirements: IdList
    adrs: IdList
    owned_paths: list[NonEmpty]


class SprintBead(Node):
    """A stage:sprint container: carries the sprint doc and its wave, and is never dispatched."""
    labels: list[str]
    description: Annotated[str, Field(pattern=DELIVERABLES)]
    acceptance_criteria: NonEmpty
    metadata: SprintMetadata

    @model_validator(mode="after")
    def labelled(self) -> SprintBead:
        found = [] if "stage:sprint" in self.labels else ["missing label stage:sprint"]
        found += [p for p in [wave_label(self.labels, self.metadata.wave)] if p]
        if found:
            raise ValueError("; ".join(found))
        return self


class ChainStepMetadata(BaseModel):
    model_config = ConfigDict(extra="allow")
    sprint_bead: NonEmpty
    sprint: NonEmpty
    stack: NonEmpty
    layer: int | NonEmpty  # the formula bakes it as a string
    phase: NonEmpty
    wave: Wave
    difficulty: Difficulty
    pr_target: NonEmpty
    dev_bead: NonEmpty | None = None
    checked_bead: NonEmpty | None = None


class ChainStep(Node):
    """A <sprint>.chain.{dev,sanity,qa} step poured from sprint-chain.formula.toml.j2."""
    id: Annotated[str, Field(pattern=r"\.chain\.(dev|sanity|qa)$")]
    labels: list[str]
    metadata: ChainStepMetadata

    @model_validator(mode="after")
    def matches_formula(self, info: ValidationInfo) -> ChainStep:
        chain, step = self.id.rsplit(".", 1)
        sprint_bead = chain.removesuffix(".chain")
        meta, found = self.metadata, []
        if self.parent_id() != chain:
            found.append(f"parent is {json.dumps(self.parent_id())}, not {chain}")
        if STEP_STAGE[step] not in self.labels:
            found.append(f"missing label {STEP_STAGE[step]}")
        found += [p for p in [wave_label(self.labels, meta.wave)] if p]
        if meta.sprint_bead != sprint_bead:
            found.append(f"metadata.sprint_bead is {json.dumps(meta.sprint_bead)}, not {sprint_bead}")
        checked = chain_ids(sprint_bead)["dev"]
        want = {"sanity": ("dev_bead", meta.dev_bead), "qa": ("checked_bead", meta.checked_bead)}.get(step)
        if want and want[1] != checked:
            found.append(f"metadata.{want[0]} is {json.dumps(want[1])}, not {checked}")
        container = ((info.context or {}).get("beads") or {}).get(sprint_bead)
        if container:
            baked = container.get("metadata") or {}
            for key in ("sprint", "stack", "layer", "phase", "wave", "difficulty", "pr_target"):
                ours = getattr(meta, key)
                if key == "layer" and str(ours) == str(baked.get(key)):
                    continue
                if ours != baked.get(key):
                    found.append(f"metadata.{key} is {json.dumps(ours)}, the sprint's is {json.dumps(baked.get(key))}")
        if found:
            raise ValueError("; ".join(found))
        return self


class SanityMetadata(BaseModel):
    model_config = ConfigDict(extra="allow")
    dev_bead: NonEmpty


def severity(bead: dict) -> str | None:
    labelled = [label.removeprefix("severity:") for label in bead.get("labels") or [] if label.startswith("severity:")]
    return (bead.get("metadata") or {}).get("severity") or next(iter(labelled), None)


class SanityBead(Bead):
    """A stage:dev-sanity bead: it blocks on the bead it checks, except the fix sanity bead of an important finding
    (parent = dev_bead = the finding), which the finding never blocks: the finding stays open for quality-mgr."""
    metadata: SanityMetadata

    @model_validator(mode="after")
    def blocks_on_dev_bead(self, info: ValidationInfo) -> SanityBead:
        dev = self.metadata.dev_bead
        parent = ((info.context or {}).get("beads") or {}).get(dev) if self.parent_id() == dev else None
        if parent is not None and severity(parent) == "important":
            if dev in self.blocks():
                raise ValueError(f"blocks edge to its important finding {dev} must be absent (dev_bead_open)")
            return self
        if dev not in self.blocks():
            raise ValueError(f"missing blocks edge to its sprint {dev}")
        return self


MODELS: dict[str, type[Node]] = {"sprint-bead": SprintBead, "chain-step": ChainStep, "sanity-bead": SanityBead,
                                 "legacy-sprint-bead": LegacySprintBead}


def problems(bead: dict, model: type[Node], beads: dict[str, dict] | None = None) -> list[str]:
    try:
        model.model_validate(bead, context={"beads": beads or {}})
    except ValidationError as exc:
        out = []
        for e in exc.errors():
            where = ".".join(str(p) for p in e["loc"])
            msg = e["msg"].removeprefix("Value error, ")
            out.append(f"{bead.get('id', '?')}: {where + ': ' if where else ''}{msg}")
        return out
    return []


def main(argv: list[str]) -> int:
    if len(argv) == 4 and argv[0] == "validate":
        beads = {b.get("id"): b for b in json.loads(Path(argv[1]).read_text())}
        index, prefix = Path(argv[2]), argv[3]
        try:
            fmt = plan_format(index)
            checks = []
            if fmt == CONTAINER:
                for row in parse_container_plan(index, prefix):
                    checks.append((row["sprint_bead"], SprintBead))
                    checks += [(row["chain"][step], ChainStep) for step in ("dev", "sanity", "qa")]
            else:
                for row in parse_legacy_plan(index, prefix):
                    checks += [(row["dev_bead_id"], LegacySprintBead), (row["sanity_bead_id"], SanityBead)]
        except RuntimeError as exc:
            print(exc)
            return 5
        found = []
        for bid, model in checks:
            if bid in beads:
                found += problems(beads[bid], model, beads)
        print("\n".join(found)) if found else None
        return 5 if found else 0
    if len(argv) == 2 and argv[0] == "export":
        for name, model in MODELS.items():
            Path(argv[1], f"{name}.schema.json").write_text(json.dumps(model.model_json_schema(), indent=2) + "\n")
        return 0
    print(__doc__, file=sys.stderr)
    return 2


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv[1:]))
    except (OSError, ValueError) as exc:
        print(f"bead_schema: {exc}", file=sys.stderr)
        raise SystemExit(2)
