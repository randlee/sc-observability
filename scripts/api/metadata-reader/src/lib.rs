//! Inspect already-built foreign metadata. This helper never invokes Cargo,
//! rustdoc, a compiler subprocess, target source analysis, or code generation.
//! The pinned compiler parses only an in-memory list of `extern crate` items;
//! its callback stops after expansion, before local analysis and codegen.
#![feature(rustc_private)]
#![cfg(test)]
extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_infer;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;
extern crate rustc_span;
extern crate rustc_trait_selection;

use rustc_driver::{Callbacks, Compilation};
use rustc_hir::def::DefKind;
use rustc_hir::def_id::DefId;
use rustc_middle::ty::{self, TyCtxt};
use rustc_trait_selection::traits::auto_trait::{AutoTraitFinder, AutoTraitResult};
use std::collections::{BTreeSet, HashSet};

struct Surface<'tcx> {
    tcx: TyCtxt<'tcx>,
    rows: BTreeSet<String>,
    active: HashSet<DefId>,
    reachable: HashSet<DefId>,
    auto_traits: Vec<DefId>,
}

impl<'tcx> Surface<'tcx> {
    fn new(tcx: TyCtxt<'tcx>) -> Self {
        let auto_traits = tcx
            .crates(())
            .iter()
            .flat_map(|krate| tcx.traits(*krate))
            .copied()
            .filter(|id| {
                tcx.trait_is_auto(*id)
                    && !tcx
                        .lookup_stability(*id)
                        .is_some_and(|s| s.level.is_unstable())
            })
            .collect();
        Self {
            tcx,
            rows: BTreeSet::new(),
            active: HashSet::new(),
            reachable: HashSet::new(),
            auto_traits,
        }
    }

    fn generics(&mut self, id: DefId, path: &str) {
        let tcx = self.tcx;
        for parameter in &tcx.generics_of(id).own_params {
            let kind = match parameter.kind {
                ty::GenericParamDefKind::Lifetime => "lifetime".to_owned(),
                ty::GenericParamDefKind::Type { has_default, .. } => {
                    if has_default {
                        format!(
                            "type={}",
                            tcx.type_of(parameter.def_id).instantiate_identity()
                        )
                    } else {
                        "type".to_owned()
                    }
                }
                ty::GenericParamDefKind::Const { has_default, .. } => {
                    let default = if has_default {
                        format!(
                            "={}",
                            tcx.const_param_default(parameter.def_id)
                                .instantiate_identity()
                        )
                    } else {
                        String::new()
                    };
                    format!(
                        "const:{}{default}",
                        tcx.type_of(parameter.def_id).instantiate_identity()
                    )
                }
            };
            self.rows.insert(format!(
                "generic {path} {} {} {kind}",
                parameter.index, parameter.name
            ));
        }
        let mut next = Some(id);
        while let Some(owner) = next {
            let predicates = tcx.predicates_of(owner);
            for (predicate, _) in predicates.predicates {
                self.rows.insert(format!("bound {path} {predicate}"));
            }
            next = predicates.parent;
        }
    }

    fn visit(&mut self, id: DefId, path: &str) {
        let tcx = self.tcx;
        let kind = tcx.def_kind(id);
        self.rows.insert(format!("item {path} {kind:?}"));
        self.reachable.insert(id);
        if !self.active.insert(id) {
            return;
        }
        match kind {
            DefKind::Mod => {
                for child in tcx.module_children(id) {
                    if child.vis.is_public()
                        && let Some(def) = child.res.opt_def_id()
                    {
                        self.visit(def, &format!("{path}::{}", child.ident.name));
                    }
                }
            }
            DefKind::Fn | DefKind::AssocFn => {
                self.generics(id, path);
                self.rows.insert(format!(
                    "signature {path} {}",
                    tcx.fn_sig(id).instantiate_identity()
                ));
                self.rows
                    .insert(format!("const {path} {}", tcx.is_const_fn(id)));
                self.rows
                    .insert(format!("async {path} {:?}", tcx.asyncness(id)));
                if kind == DefKind::AssocFn {
                    self.rows
                        .insert(format!("default {path} {:?}", tcx.defaultness(id)));
                }
            }
            DefKind::Struct | DefKind::Enum | DefKind::Union => {
                self.generics(id, path);
                let adt = tcx.adt_def(id);
                self.rows.insert(format!(
                    "non_exhaustive {path} {}",
                    adt.is_variant_list_non_exhaustive()
                ));
                let repr = adt.repr();
                self.rows.insert(format!(
                    "repr {path} c={} transparent={} simd={} int={:?} pack={:?} align={:?}",
                    repr.c(),
                    repr.transparent(),
                    repr.simd(),
                    repr.int,
                    repr.pack,
                    repr.align
                ));
                for (index, variant) in adt.variants().iter_enumerated() {
                    let variant_path = if adt.is_enum() {
                        format!("{path}::{}", variant.name)
                    } else {
                        path.to_owned()
                    };
                    self.rows.insert(format!(
                        "variant {variant_path} non_exhaustive={} constructor={:?}",
                        variant.is_field_list_non_exhaustive(),
                        variant.ctor_kind()
                    ));
                    if adt.is_enum() {
                        self.rows.insert(format!(
                            "discriminant {variant_path} {}",
                            adt.discriminant_for_variant(tcx, index).val
                        ));
                    }
                    self.rows.insert(format!(
                        "private_fields {variant_path} {}",
                        variant.fields.iter().any(|field| !field.vis.is_public())
                    ));
                    for field in &variant.fields {
                        if field.vis.is_public() {
                            self.rows.insert(format!(
                                "field {variant_path}::{} {}",
                                field.name,
                                tcx.type_of(field.did).instantiate_identity()
                            ));
                        }
                    }
                }
                for implementation in tcx.inherent_impls(id) {
                    for item in tcx.associated_items(*implementation).in_definition_order() {
                        if tcx.visibility(item.def_id).is_public() {
                            self.visit(item.def_id, &format!("{path}::{}", item.name()));
                        }
                    }
                }
                self.synthetic_impls(id, path);
            }
            DefKind::Trait => {
                self.generics(id, path);
                self.rows.insert(format!(
                    "trait {path} auto={} safety={:?}",
                    tcx.trait_is_auto(id),
                    tcx.trait_def(id).safety
                ));
                for item in tcx.associated_items(id).in_definition_order() {
                    self.visit(item.def_id, &format!("{path}::{}", item.name()));
                }
            }
            DefKind::TyAlias | DefKind::Const | DefKind::Static { .. } => {
                self.generics(id, path);
                self.rows.insert(format!(
                    "type {path} {}",
                    tcx.type_of(id).instantiate_identity()
                ));
            }
            DefKind::AssocTy | DefKind::AssocConst => {
                self.generics(id, path);
                let defaultness = tcx.defaultness(id);
                if kind == DefKind::AssocTy {
                    for predicate in tcx.item_bounds(id).instantiate_identity() {
                        self.rows
                            .insert(format!("associated_bound {path} {predicate}"));
                    }
                }
                self.rows.insert(format!("default {path} {defaultness:?}"));
                if kind == DefKind::AssocConst || defaultness.has_value() {
                    self.rows.insert(format!(
                        "type {path} {}",
                        tcx.type_of(id).instantiate_identity()
                    ));
                }
            }
            DefKind::Macro(_) | DefKind::Ctor(..) | DefKind::Variant => {}
            _ => panic!("unsupported public item {path}: {kind:?}"),
        }
        self.active.remove(&id);
    }

    fn synthetic_impls(&mut self, id: DefId, path: &str) {
        let tcx = self.tcx;
        let target_ty = tcx.type_of(id).instantiate_identity();
        let finder = AutoTraitFinder::new(tcx);
        for trait_id in &self.auto_traits {
            let result = finder.find_auto_trait_generics(
                target_ty,
                ty::TypingEnv::non_body_analysis(tcx, id),
                *trait_id,
                |info| {
                    info.full_user_env
                        .caller_bounds()
                        .iter()
                        .map(|p| p.to_string())
                        .collect::<BTreeSet<_>>()
                },
            );
            match result {
                AutoTraitResult::PositiveImpl(bounds) => {
                    self.rows.insert(format!(
                        "auto {path} {} bounds={bounds:?}",
                        tcx.def_path_str(*trait_id)
                    ));
                }
                AutoTraitResult::NegativeImpl => {
                    self.rows
                        .insert(format!("auto {path} !{}", tcx.def_path_str(*trait_id)));
                }
                AutoTraitResult::ExplicitImpl => {}
            }
        }
        // Dependency blanket implementations are not declarations made by this
        // package. They are transitive implementation detail and can change
        // with a dependency without changing the supported surface. Retain
        // compiler-evaluated auto-trait capabilities above and crate-owned
        // implementations below; omit dependency blanket expansion entirely.
    }

    fn implementations(&mut self, krate: rustc_hir::def_id::CrateNum) {
        let tcx = self.tcx;
        for id in tcx.trait_impls_in_crate(krate) {
            let trait_ref = tcx.impl_trait_ref(*id).instantiate_identity();
            if !tcx.visibility(trait_ref.def_id).is_public()
                || !tcx.visible_parent_map(()).contains_key(&trait_ref.def_id)
            {
                continue;
            }
            let public_self = match trait_ref.self_ty().kind() {
                ty::Adt(adt, _) => self.reachable.contains(&adt.did()),
                ty::Param(_) => true,
                _ => self.reachable.contains(&trait_ref.def_id),
            };
            if !public_self {
                continue;
            }
            let label = format!("{} {:?}", trait_ref, tcx.impl_polarity(*id));
            self.rows.insert(format!("impl {label}"));
            self.generics(*id, &label);
            for item in tcx.associated_items(*id).in_definition_order() {
                self.visit(item.def_id, &format!("{label}::{}", item.name()));
            }
        }
    }
}

struct Reader {
    name: String,
    rows: Option<BTreeSet<String>>,
}
impl Callbacks for Reader {
    fn config(&mut self, config: &mut rustc_interface::interface::Config) {
        config.input = rustc_session::config::Input::Str {
            name: rustc_span::FileName::Custom("api_metadata_reader".into()),
            input: format!("extern crate {};", self.name),
        };
    }
    fn after_expansion<'tcx>(
        &mut self,
        _: &rustc_interface::interface::Compiler,
        tcx: TyCtxt<'tcx>,
    ) -> Compilation {
        let krate = tcx
            .crates(())
            .iter()
            .find(|id| tcx.crate_name(**id).as_str() == self.name)
            .expect("requested compiled library is loaded");
        let mut surface = Surface::new(tcx);
        surface.visit(krate.as_def_id(), &self.name);
        surface.implementations(*krate);
        self.rows = Some(surface.rows);
        Compilation::Stop
    }
}

#[test]
fn reader_setup() {
    assert!(std::path::Path::new(env!("API_RUST_SYSROOT")).is_dir());
}

/// Executed by the API unit checker only AFTER the normal Cargo invocation has
/// finished, using its compiler-artifact filenames. No directory guessing.
#[test]
#[ignore = "invoked with exact current artifacts by scripts/api/run_unit_tests.py"]
fn metadata_dump() {
    let request = std::env::var("SC_API_READER_REQUEST").expect("artifact request");
    let requests: Vec<serde_json::Value> = serde_json::from_str(&request).expect("JSON request");
    for request in requests {
        let name = request["lib_name"]
            .as_str()
            .expect("library name")
            .to_owned();
        assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));
        let artifact = request["artifact"].as_str().expect("artifact filename");
        assert!(
            std::path::Path::new(artifact).is_file(),
            "missing built artifact {artifact}"
        );
        let mut reader = Reader {
            name: name.clone(),
            rows: None,
        };
        let args = vec![
            "api-reader".into(),
            "--crate-type=lib".into(),
            "--edition=2024".into(),
            "api_metadata_reader.rs".into(),
            "--sysroot".into(),
            env!("API_RUST_SYSROOT").into(),
            "--extern".into(),
            format!("{name}={artifact}"),
            "-L".into(),
            format!(
                "dependency={}",
                std::path::Path::new(artifact)
                    .parent()
                    .expect("artifact parent")
                    .display()
            ),
        ];
        rustc_driver::run_compiler(&args, &mut reader);
        println!(
            "SC_API_ROWS={}",
            serde_json::json!({"artifact": artifact, "rows": reader.rows.expect("metadata callback ran")})
        );
    }
}
