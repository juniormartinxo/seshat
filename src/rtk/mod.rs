//! Token-saving filters and diff condensation, vendored from rtk-ai/rtk.
//!
//! Source: <https://github.com/rtk-ai/rtk> (v0.35.0)
//! Copyright 2024 Patrick Szymkowiak
//! Licensed under the Apache License, Version 2.0.
//!
//! Only the pure filter + diff-condensation logic was vendored (no CLI,
//! telemetry, or I/O). Upstream improvements must be re-synced manually.
//! The public API is re-exported from the submodules below so call sites
//! do not need to know about the vendor layout.

pub mod diff;
pub mod filter;

pub use diff::condense_unified_diff;
pub use filter::{get_filter, FilterLevel, FilterStrategy, Language};
