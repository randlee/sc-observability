//! Render a rustdoc JSON document into the public API rows compared by
//! `scripts/ci/public_api_parity.py` (ADR-022).
//!
//! The renderer keeps auto-trait and auto-derived implementations and omits
//! blanket implementations (`impl<T> Trait for T` materialised from whichever
//! dependency crates a target happens to compile, e.g. `objc2` behind Tauri on
//! macOS; they describe the dependency set, not the published API). It
//! refuses rustdoc JSON whose `format_version` differs from the pinned
//! `rustdoc-types` schema, and reports every referenced item id that the
//! document neither defines nor names through its external `paths` table, so
//! an incomplete extraction cannot pass as an equal API.

use std::path::PathBuf;
use std::process::ExitCode;

use rustdoc_types::{Crate, Id};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct FormatHeader {
    format_version: u32,
}

#[derive(Serialize)]
struct Rendering {
    schema: u32,
    renderer: RendererIdentity,
    /// Referenced ids that are neither defined locally nor resolvable through
    /// `paths`: the surface is incomplete and must not be compared.
    unresolved_item_ids: Vec<u32>,
    /// Referenced external items (for example `core::marker::Send`) that the
    /// document names through `paths` without defining; this is normal.
    external_item_ids: usize,
    rows: Vec<String>,
}

#[derive(Serialize)]
struct RendererIdentity {
    public_api: &'static str,
    rustdoc_types: &'static str,
    format_version: u32,
    minimum_nightly: &'static str,
    impls: ImplPolicy,
}

/// Which implementation classes the rendered rows include; part of the
/// renderer identity so cells rendered under another policy never compare.
#[derive(Serialize)]
struct ImplPolicy {
    blanket: bool,
    auto_trait: bool,
    auto_derived: bool,
}

const IMPL_POLICY: ImplPolicy = ImplPolicy {
    blanket: false,
    auto_trait: true,
    auto_derived: true,
};

const PUBLIC_API_VERSION: &str = "0.52.2";
const RUSTDOC_TYPES_VERSION: &str = "0.59.0";

fn render(path: &PathBuf) -> Result<Rendering, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read rustdoc JSON {}: {error}", path.display()))?;
    let header: FormatHeader = serde_json::from_str(&text)
        .map_err(|error| format!("rustdoc JSON {} lacks a format_version: {error}", path.display()))?;
    if header.format_version != rustdoc_types::FORMAT_VERSION {
        return Err(format!(
            "rustdoc JSON format_version {} differs from the pinned rustdoc-types format {}; \
             align scripts/ci/public-api-toolchain with the renderer pins",
            header.format_version,
            rustdoc_types::FORMAT_VERSION
        ));
    }
    let krate: Crate = serde_json::from_str(&text)
        .map_err(|error| format!("rustdoc JSON {} does not deserialize: {error}", path.display()))?;
    let api = public_api::Builder::from_rustdoc_json(path)
        .omit_blanket_impls(!IMPL_POLICY.blanket)
        .omit_auto_trait_impls(!IMPL_POLICY.auto_trait)
        .omit_auto_derived_impls(!IMPL_POLICY.auto_derived)
        .include_function_parameter_names(false)
        .sorted(true)
        .build()
        .map_err(|error| format!("public-api rendering failed for {}: {error}", path.display()))?;
    let mut unresolved_item_ids = Vec::new();
    let mut external_item_ids = 0usize;
    let mut seen = std::collections::BTreeSet::new();
    for id in api.missing_item_ids().copied() {
        if !seen.insert(id) {
            continue;
        }
        match krate.paths.get(&Id(id)) {
            Some(summary) if summary.crate_id != 0 => external_item_ids += 1,
            _ => unresolved_item_ids.push(id),
        }
    }
    let rows: Vec<String> = api.items().map(ToString::to_string).collect();
    Ok(Rendering {
        schema: 1,
        renderer: RendererIdentity {
            public_api: PUBLIC_API_VERSION,
            rustdoc_types: RUSTDOC_TYPES_VERSION,
            format_version: rustdoc_types::FORMAT_VERSION,
            minimum_nightly: public_api::MINIMUM_NIGHTLY_RUST_VERSION,
            impls: IMPL_POLICY,
        },
        unresolved_item_ids,
        external_item_ids,
        rows,
    })
}

fn main() -> ExitCode {
    let mut args = std::env::args_os().skip(1);
    let (Some(path), None) = (args.next(), args.next()) else {
        eprintln!("usage: public-api-surface-renderer <rustdoc-json-path>");
        return ExitCode::from(2);
    };
    match render(&PathBuf::from(path)) {
        Ok(rendering) => {
            let encoded = serde_json::to_string(&rendering).expect("rendering serializes");
            println!("{encoded}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::from(3)
        }
    }
}
