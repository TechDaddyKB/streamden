//! StreamDen protocol and contracts (Spec 02, `docs/spec/02-protocol.md`).
//!
//! Every public wire and storage type lives here. Changing this crate needs an ADR and a dedicated
//! contract card (AGENTS.md, single-writer files). Types, the catalog and `cargo xtask codegen` arrive
//! with the Spec 02 implementation cards; this scaffold only fixes the crate boundary.

/// Version of this crate, reported by binaries in their startup log.
pub const CRATE_VERSION: &str = env!("CARGO_PKG_VERSION");
