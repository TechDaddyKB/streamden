//! `streamdend`: the StreamDen engine without a tray (architecture §1).
//!
//! Scaffold only: prints its version and exits. The engine loop arrives with Spec 03 and Spec 06.

use den_server::den_core::den_protocol;

fn main() {
    println!(
        "streamdend {} (den-protocol {})",
        env!("CARGO_PKG_VERSION"),
        den_protocol::CRATE_VERSION
    );
}
