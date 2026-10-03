"""Focused fixture coverage for compatibility registry signature validation."""
import json
import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from compatibility_registry import (  # noqa: E402
    BASELINE_COMMIT,
    DEPRECATED_OWNER_BASELINE,
    deprecated_owner_exception_records,
    deprecated_owner_names,
    has_placeholder_baseline_signature,
    is_allowed_compat_reference_source,
    is_compat_source_path,
    validate_compatibility_source_boundary,
    validate_contract_signatures,
    validate_trait_impl_contracts,
    validate_trait_slot_contracts,
)

REPO_ROOT = Path(__file__).resolve().parents[3]


def baseline_source(relative: str) -> str | None:
    result = subprocess.run(
        ["git", "-C", str(REPO_ROOT), "show", f"{BASELINE_COMMIT}:{relative}"],
        capture_output=True, text=True, check=False)
    return result.stdout if result.returncode == 0 else None


def exceptions(relative: str, *symbols: str) -> dict:
    """Registry fragment with one structured deprecated-owner exception record."""
    return {"deprecated_owner_exceptions": [{
        "file": relative, "deprecated_symbols": list(symbols),
        "reason": "fixture reason", "removal_point": "fixture removal point"}]}


def validate_source(relative: str, contents: str, registry=None, baseline=None) -> None:
    """Validate one temporary source file against a registry fragment and baseline."""
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary)
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
        validate_compatibility_source_boundary(root, [path], registry or {}, baseline or {})


class BaselineSignatureTests(unittest.TestCase):
    def test_known_nominal_identity_placeholder_is_rejected(self):
        self.assertTrue(has_placeholder_baseline_signature(
            "released public nominal identity `sc_observability_types::IdentityError`"))

    def test_concrete_signature_is_accepted(self):
        self.assertFalse(has_placeholder_baseline_signature(
            "pub struct IdentityError { code: ErrorCode, message: String }"))

    def test_prefix_match_is_exact(self):
        self.assertFalse(has_placeholder_baseline_signature(
            "released public identity for sc_observability_types::IdentityError"))


class ContractSignatureTests(unittest.TestCase):
    SIGNATURE = "pub struct IdentityError(pub Box<ErrorContext>);"

    def row(self, **overrides):
        row = {"symbol": "sc_observability_types::IdentityError", "treatment": "unchanged_alias",
               "baseline_signature": self.SIGNATURE, "canonical_signature": self.SIGNATURE,
               "removable_paths": [],
               "removal_rationale": "fixture row intentionally has no removable path"}
        row.update(overrides)
        return row

    def test_equal_unchanged_alias_is_accepted(self):
        validate_contract_signatures([self.row(removal_rationale=None)])

    def test_unchanged_alias_with_different_signatures_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "unchanged_alias baseline and canonical signatures differ"):
            validate_contract_signatures([self.row(canonical_signature="pub enum IdentityError { Process }")])

    def test_different_signatures_are_accepted_for_existing_pair(self):
        validate_contract_signatures([self.row(
            treatment="existing_pair", canonical_signature="pub enum IdentityError { Process }",
            removal_rationale="the retained declaration is not removable")])

    def test_empty_removable_paths_with_rationale_is_accepted(self):
        validate_contract_signatures([self.row(
            treatment="existing_pair", canonical_signature="pub enum IdentityError { Process }",
            removal_rationale="the retained declaration is not removable")])

    def test_empty_removable_paths_without_rationale_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "empty removable_paths.*unchanged_alias.*removal_rationale"):
            validate_contract_signatures([self.row(
                treatment="existing_pair", canonical_signature="pub enum IdentityError { Process }",
                removal_rationale=None)])

    def test_empty_removable_paths_with_blank_rationale_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "empty removable_paths.*unchanged_alias.*removal_rationale"):
            validate_contract_signatures([self.row(
                treatment="existing_pair", canonical_signature="pub enum IdentityError { Process }",
                removal_rationale=" \t ")])

    def test_placeholder_canonical_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder canonical contract"):
            validate_contract_signatures([self.row(
                treatment="existing_pair",
                canonical_signature="released public nominal identity `v2::IdentityError`")])

    def test_whitespace_only_canonical_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder canonical contract"):
            validate_contract_signatures([self.row(
                treatment="existing_pair", canonical_signature="   ")])

    def test_padded_placeholder_canonical_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder canonical contract"):
            validate_contract_signatures([self.row(
                treatment="existing_pair",
                canonical_signature=" released public nominal identity `v2::IdentityError` ")])

    def test_empty_canonical_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder canonical contract"):
            validate_contract_signatures([self.row(treatment="existing_pair", canonical_signature="")])

    def test_missing_canonical_signature_is_left_to_the_restoration_rule(self):
        validate_contract_signatures([self.row(treatment="restoration", canonical_signature=None)])

    def test_placeholder_baseline_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder baseline contract"):
            validate_contract_signatures([self.row(
                baseline_signature="released public nominal identity `IdentityError`")])

    def test_whitespace_only_baseline_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder baseline contract"):
            validate_contract_signatures([self.row(baseline_signature="   ")])

    def test_padded_placeholder_baseline_signature_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "placeholder baseline contract"):
            validate_contract_signatures([self.row(
                baseline_signature=" released public nominal identity `IdentityError` ")])


class TraitSlotContractTests(unittest.TestCase):
    PATH = "crates/sc-observability-types/src/observation_v2.rs"

    def row(self, **overrides):
        row = {"symbol": "sc_observability_types::ProcessIdentityResolver::resolve", "treatment": "new_adapter",
               "baseline_signature": "fn resolve(&self) -> Result<ProcessIdentity, crate::IdentityError>",
               "canonical_signature": "fn resolve(&self) -> Result<ProcessIdentity, crate::v2::IdentityError>",
               "canonical_source": {"path": self.PATH}, "removable_paths": [],
               "removal_rationale": "the released trait is retained for the supported release line"}
        row.update(overrides)
        return row

    def test_qualified_signature_change_is_accepted(self):
        validate_trait_slot_contracts([self.row()])

    def test_new_adapter_with_equal_signatures_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "new_adapter baseline and canonical signatures are equal"):
            validate_trait_slot_contracts([self.row(canonical_signature=self.row()["baseline_signature"])])

    def test_equal_signatures_are_accepted_for_unchanged_alias(self):
        validate_trait_slot_contracts([self.row(
            treatment="unchanged_alias", canonical_signature=self.row()["baseline_signature"])])

    def test_removable_canonical_source_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "removable_paths contains its canonical source"):
            validate_trait_slot_contracts([self.row(removable_paths=[self.PATH])])

    def test_removable_path_is_matched_exactly(self):
        validate_trait_slot_contracts([self.row(removable_paths=[self.PATH + ".bak", "src/observation_v2.rs"])])


class TraitImplContractTests(unittest.TestCase):
    def record(self, implementation, **overrides):
        owner = implementation.rsplit("::", 1)[-1]
        record = {
            "implementation": implementation,
            "baseline_declaration": f"impl crate::typed::TypedLogSink for {owner}",
            "current_declaration": f"impl TypedLogSink for {owner}",
            "baseline_source": {"revision": BASELINE_COMMIT, "path": "crates/sc-observability/src/sinks.rs", "owner": owner},
            "current_source": {"revision": "selected_head", "path": "crates/sc-observability/src/compat.rs", "owner": owner},
            "conversion": "delegates to the canonical sink and converts its context into LogSinkFailure",
            "removable_paths": ["crates/sc-observability/src/compat.rs"],
            "removal_rationale": "remove this released trait implementation with compat.rs after the 1.x surface retires",
        }
        record.update(overrides)
        return record

    def records(self):
        return [
            self.record("sc_observability::typed::TypedLogSink for sc_observability::JsonlFileSink"),
            self.record("sc_observability::typed::TypedLogSink for sc_observability::ConsoleSink"),
        ]

    def test_released_builtin_impl_records_are_accepted(self):
        validate_trait_impl_contracts(self.records())

    def test_missing_released_builtin_impl_record_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "missing="):
            validate_trait_impl_contracts(self.records()[:1])

    def test_duplicate_released_builtin_impl_record_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "duplicate compatibility trait-impl"):
            validate_trait_impl_contracts([self.records()[0], self.records()[0]])

    def test_malformed_released_builtin_impl_record_is_rejected(self):
        malformed = self.records()
        malformed[0].pop("conversion")
        with self.assertRaisesRegex(ValueError, "malformed compatibility trait-impl"):
            validate_trait_impl_contracts(malformed)


class CompatibilitySourcePathTests(unittest.TestCase):
    def test_compat_file_is_accepted(self):
        self.assertTrue(is_compat_source_path(
            "crates/sc-observability-otlp/src/compat.rs"))

    def test_compat_module_descendant_is_accepted(self):
        self.assertTrue(is_compat_source_path(
            "crates/sc-observability-otlp/src/compat/mod.rs"))

    def test_compatibility_file_is_rejected(self):
        self.assertFalse(is_compat_source_path(
            "crates/sc-observability-otlp/src/compatibility.rs"))

    def test_compat_extra_file_is_rejected(self):
        self.assertFalse(is_compat_source_path(
            "crates/sc-observability-otlp/src/compat_extra.rs"))

    def test_compatibility_module_descendant_is_rejected(self):
        self.assertFalse(is_compat_source_path(
            "crates/sc-observability-otlp/src/compatibility/mod.rs"))


class CompatibilityReferenceGuardTests(unittest.TestCase):
    def test_registered_facade_root_is_accepted(self):
        exceptions = {"crates/sc-observability-log/src/lib.rs"}
        self.assertTrue(is_allowed_compat_reference_source(
            "crates/sc-observability-log/src/lib.rs", exceptions))

    def test_unregistered_root_is_rejected(self):
        exceptions = {"crates/sc-observability-log/src/lib.rs"}
        self.assertFalse(is_allowed_compat_reference_source(
            "crates/sc-observability/src/lib.rs", exceptions))

    def test_non_root_file_is_rejected_even_when_its_root_is_registered(self):
        exceptions = {"crates/sc-observability-log/src/lib.rs"}
        self.assertFalse(is_allowed_compat_reference_source(
            "crates/sc-observability-log/src/handle.rs", exceptions))


class CompatibilitySourceBoundaryTests(unittest.TestCase):
    def source_file(self, root: Path, relative: str, contents: str) -> Path:
        path = root / relative
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(contents, encoding="utf-8")
        return path

    def test_compat_file_and_module_descendant_are_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            sources = [self.source_file(root, relative, "#[deprecated]\nuse crate::compat::Legacy;\n")
                       for relative in ("crates/example/src/compat.rs", "crates/example/src/compat/mod.rs")]
            validate_compatibility_source_boundary(root, sources, {})

    def test_canonical_source_importing_compat_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.source_file(
                root, "crates/example/src/runtime.rs", "use crate::compat::Legacy;\n")

            with self.assertRaisesRegex(ValueError, "canonical source imports compatibility module"):
                validate_compatibility_source_boundary(root, [source], {})

    def test_unrecorded_deprecated_owner_is_rejected(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = self.source_file(
                root, "crates/example/src/runtime.rs", "#[deprecated]\npub fn old() {}\n")

            with self.assertRaisesRegex(ValueError, "deprecated owner is outside compat"):
                validate_compatibility_source_boundary(root, [source], {})

    def test_recorded_root_reexport_is_accepted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            relative = "crates/example/src/lib.rs"
            source = self.source_file(root, relative, "pub use crate::compat::Legacy;\n")

            validate_compatibility_source_boundary(
                root, [source], {"compat_root_reexport_exceptions": [relative]})

    def validate(self, relative: str, contents: str, registry=None, baseline=None):
        validate_source(relative, contents, registry, baseline)

    def test_recorded_deprecated_owner_is_accepted(self):
        relative = "crates/example/src/runtime.rs"
        self.validate(relative, "#[deprecated]\npub fn old() {}\n",
                      exceptions(relative, "old"), {relative: ("old",)})

    def test_deprecated_owner_beyond_baseline_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: new_owner"):
            self.validate(relative, "#[deprecated]\npub fn old() {}\n#[deprecated]\npub fn new_owner() {}\n",
                          exceptions(relative, "old"), {relative: ("old",)})

    def test_renamed_deprecated_owner_within_baseline_count_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: emit_canonical"):
            self.validate(relative, "#[deprecated]\npub fn emit_canonical() {}\n",
                          exceptions(relative, "emit"), {relative: ("emit", "flush")})

    def test_repeated_deprecated_owner_beyond_baseline_count_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        contents = "impl A {\n#[deprecated]\npub fn new() {}\n}\nimpl B {\n#[deprecated]\npub fn new() {}\n}\n"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: new"):
            self.validate(relative, contents,
                          exceptions(relative, "new"), {relative: ("new",)})

    def test_new_same_line_owner_before_baseline_named_item_is_rejected(self):
        relative = "crates/example/src/lib.rs"
        contents = "#[deprecated] pub fn brand_new() {}\npub fn max_age_days() {}\n"
        with self.assertRaisesRegex(ValueError, "exceeds v1.4.1 baseline: .*: brand_new"):
            self.validate(
                relative, contents, exceptions(relative, "max_age_days"), {relative: ("max_age_days",)})

    def test_exception_without_baseline_is_rejected(self):
        relative = "crates/example/src/runtime.rs"
        with self.assertRaisesRegex(ValueError, "exception has no v1.4.1 baseline"):
            self.validate(relative, "#[deprecated]\npub fn old() {}\n",
                          exceptions(relative, "old"), {})

    def test_suffixed_compat_path_is_not_a_compat_reference(self):
        self.validate("crates/example/src/runtime.rs", "use crate::foo_compat::Legacy;\n")

    def test_relative_compat_path_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "canonical source imports compatibility module"):
            self.validate("crates/example/src/runtime.rs", "use super::compat::Legacy;\n")

    def test_aliased_compat_module_import_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "canonical source imports compatibility module"):
            self.validate("crates/example/src/runtime.rs", "use crate::compat as legacy;\n")

    def test_compat_module_declared_outside_crate_root_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "compatibility module declared outside a crate root"):
            self.validate("crates/example/src/runtime.rs", "mod compat;\n")

    def test_compat_module_declared_at_crate_root_is_accepted(self):
        self.validate("crates/example/src/lib.rs", "mod compat;\n")

    def test_compatibility_module_declaration_is_not_compat(self):
        self.validate("crates/example/src/runtime.rs", "mod compatibility;\n")


class DeprecatedOwnerExceptionRecordTests(unittest.TestCase):
    RELATIVE = "crates/example/src/runtime.rs"
    BASELINE = {RELATIVE: ("old", "legacy")}

    def records(self, **overrides):
        record = exceptions(self.RELATIVE, "old")["deprecated_owner_exceptions"][0]
        record.update(overrides)
        return {"deprecated_owner_exceptions": [record]}

    def reject(self, registry, message):
        with self.assertRaisesRegex(ValueError, message):
            deprecated_owner_exception_records(registry, self.BASELINE)

    def test_complete_record_returns_its_symbols(self):
        self.assertEqual(deprecated_owner_exception_records(self.records(), self.BASELINE),
                         {self.RELATIVE: ("old",)})

    def test_bare_path_entry_is_rejected(self):
        self.reject({"deprecated_owner_exceptions": [self.RELATIVE]}, "malformed deprecated owner exception record")

    def test_missing_and_extra_fields_are_rejected(self):
        record = self.records()["deprecated_owner_exceptions"][0]
        del record["reason"]
        self.reject({"deprecated_owner_exceptions": [record]}, "malformed deprecated owner exception record")
        self.reject(self.records(approved_by="nobody"), "malformed deprecated owner exception record")

    def test_duplicate_file_record_is_rejected(self):
        registry = self.records()
        registry["deprecated_owner_exceptions"].append(dict(registry["deprecated_owner_exceptions"][0]))
        self.reject(registry, "duplicate deprecated owner exception")

    def test_blank_reason_and_removal_point_are_rejected(self):
        self.reject(self.records(reason="  "), "blank reason")
        self.reject(self.records(removal_point=""), "blank removal_point")

    def test_empty_symbol_list_is_rejected(self):
        self.reject(self.records(deprecated_symbols=[]), "must name its deprecated symbols")

    def test_symbol_outside_baseline_is_rejected(self):
        self.reject(self.records(deprecated_symbols=["old", "invented"]),
                    "declares symbols outside the v1.4.1 baseline: .*: invented")

    def test_declared_symbol_missing_from_source_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "exception symbols differ from source"):
            validate_source(
                self.RELATIVE, "#[deprecated]\npub fn old() {}\n",
                exceptions(self.RELATIVE, "old", "legacy"), self.BASELINE)

    def test_undeclared_baseline_owner_in_source_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "exception symbols differ from source"):
            validate_source(
                self.RELATIVE, "#[deprecated]\npub fn old() {}\n#[deprecated]\npub fn legacy() {}\n",
                exceptions(self.RELATIVE, "old"), self.BASELINE)

    def test_record_for_file_without_deprecated_owners_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "names a file without deprecated owners"):
            validate_source(
                self.RELATIVE, "pub fn old() {}\n", exceptions(self.RELATIVE, "old"), self.BASELINE)

    def test_repository_records_match_their_source(self):
        registry = json.loads((REPO_ROOT / "docs/compatibility/registry.json").read_text(encoding="utf-8"))
        records = deprecated_owner_exception_records(registry)
        self.assertEqual(sorted(records), [
            "crates/sc-observability-types/src/errors.rs",
            "crates/sc-observability/src/lib.rs",
        ])
        for relative, symbols in records.items():
            with self.subTest(relative=relative):
                source = (REPO_ROOT / relative).read_text(encoding="utf-8")
                self.assertEqual(deprecated_owner_names(source), list(symbols))


class DeprecatedOwnerNameTests(unittest.TestCase):
    def test_multiline_attribute_and_preamble_resolve_to_the_owner(self):
        contents = (
            "#[deprecated(\n    since = \"1.4.0\",\n    note = \"use v2\"\n)]\n"
            "/// Legacy error.\n#[derive(Debug)]\npub struct IdentityError(u8);\n"
            "pub struct Config {\n    #[deprecated]\n    pub max_age_days: u32,\n}\n")
        self.assertEqual(deprecated_owner_names(contents), ["IdentityError", "max_age_days"])

    def test_same_line_owner_mid_file_is_taken_from_the_attribute_line(self):
        contents = "#[deprecated] pub fn brand_new() {}\npub fn max_age_days() {}\n"
        self.assertEqual(deprecated_owner_names(contents), ["brand_new"])

    def test_same_line_owner_at_end_of_file_is_taken_from_the_attribute_line(self):
        contents = "pub fn current() {}\n#[deprecated(note = \"use v2\")] pub struct Legacy;"
        self.assertEqual(deprecated_owner_names(contents), ["Legacy"])

    def test_recorded_baseline_matches_pinned_release_source(self):
        for relative, names in DEPRECATED_OWNER_BASELINE.items():
            with self.subTest(relative=relative):
                source = baseline_source(relative)
                if source is None:
                    if os.environ.get("CI"):
                        self.fail(f"pinned commit {BASELINE_COMMIT} is not in this CI clone: {relative}")
                    self.skipTest(f"pinned commit {BASELINE_COMMIT} is not in this local clone")
                self.assertEqual(deprecated_owner_names(source), list(names))


if __name__ == "__main__":
    unittest.main()
