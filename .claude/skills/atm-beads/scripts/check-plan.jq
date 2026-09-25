# Gate for a rendered bead plan: run on the import JSONL before `bd import`.
#   jq -rs -f scripts/check-plan.jq plan.jsonl   # prints problems, exit 5 if any
def need($c; $msg): if $c then empty else $msg end;
def blank: . == null or . == "" or . == [];

. as $all
| [ $all[] | select(.issue_type == "epic" or .issue_type == "feature") ] as $roots
| [ $all[] | select(.issue_type == "task") ] as $sprints
| ($roots[0] // {}) as $root
| ($root.metadata.phase // "") as $p
| [
    need($roots | length == 1; "plan needs exactly one epic or feature root, found \($roots | length)"),
    need($p | test("^[a-z]+$"); "phase id \($p | tojson) is not lower-case letters (a..z, aa, ab, ...)"),
    need($root.title // "" | startswith("phase-\($p): "); "root title must start with phase-\($p): "),
    ( $sprints[] as $s | $s.metadata as $m | "\($s.id)" as $id
      | need($m.phase == $p; "\($id): phase \($m.phase | tojson) is not the plan phase \($p | tojson)"),
        need($m.sprint | test("^\($p)-[1-9][0-9]*$"); "\($id): sprint \($m.sprint | tojson) is not \($p)-<n>"),
        need($m.branch | startswith("sprint/\($m.sprint)-"); "\($id): branch \($m.branch | tojson) is not sprint/\($m.sprint)-<slug>"),
        need(($s.labels // []) | index("phase-\($p)"); "\($id): missing label phase-\($p)"),
        need([$s.dependencies[] | select(.type == "parent-child" and .depends_on_id == $root.id)] | length == 1; "\($id): not a child of the plan root \($root.id)"),
        ( ["assignee", "description", "design", "acceptance_criteria"][] as $f
          | need($s[$f] | blank | not; "\($id): empty \($f)") ),
        ( ["stack", "layer", "branch", "pr_target", "worktree", "relation", "closure_type", "target_boundary", "owned_paths"][] as $k
          | need($m[$k] | blank | not; "\($id): empty metadata.\($k)") ),
        ( if $m.layer == 1 then
            need($m.pr_target == $root.metadata.integration_branch; "\($id): layer 1 pr_target must be \($root.metadata.integration_branch)")
          else
            [ $sprints[] | select(.metadata.stack == $m.stack and .metadata.layer == $m.layer - 1) ] as $below
            | if ($below | length) != 1 then "\($id): stack \($m.stack) has no single layer \($m.layer - 1)"
              else
                need($m.pr_target == $below[0].metadata.branch; "\($id): pr_target must be \($below[0].metadata.branch) (layer below)"),
                need([$s.dependencies[] | select(.type == "blocks" and .depends_on_id == $below[0].id)] | length == 1; "\($id): must be blocked by \($below[0].id) (layer below)")
              end
          end ) ),
    ( $sprints | group_by([.metadata.stack, .metadata.layer])[] | select(length > 1)
      | "stack \(.[0].metadata.stack) layer \(.[0].metadata.layer) claimed by \(map(.id) | join(", "))" )
  ]
| if length == 0 then "plan ok: \($sprints | length) sprint beads under \($root.id)" else (.[], ("\(length) problem(s)" | halt_error(5))) end
