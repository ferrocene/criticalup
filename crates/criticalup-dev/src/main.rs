// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::keys::newtypes::PublicKeyBytes;
use criticaltrust::keys::{KeyAlgorithm, KeyRole, PublicKey};

fn main() {
    let whitelabel = criticalup_cli::WhitelabelConfig {
        name: "criticalup-dev",
        http_user_agent: concat!("criticalup/", env!("CARGO_PKG_VERSION"), " (dev)"),
        download_server_url: "https://criticalup-downloads-dev.ferrocene.dev".into(),
        customer_portal_url: "https://customers-dev.ferrocene.dev".into(),
        trust_root: PublicKey {
            role: KeyRole::Root,
            algorithm: KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            expiry: None,
            public: PublicKeyBytes::borrowed(&[
                48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206, 61, 3, 1,
                7, 3, 66, 0, 4, 4, 66, 130, 231, 244, 177, 180, 109, 240, 145, 92, 154, 42, 34, 40,
                21, 109, 38, 147, 239, 19, 129, 179, 54, 221, 145, 127, 59, 125, 173, 253, 205,
                141, 183, 30, 200, 109, 54, 8, 135, 123, 21, 221, 154, 198, 91, 217, 137, 181, 90,
                76, 144, 142, 231, 13, 92, 11, 9, 224, 176, 32, 177, 178, 237,
            ]),
        },
        test_mode: false,
    };

    let args = std::env::args_os().collect::<Vec<_>>();
    std::process::exit(criticalup_cli::main(whitelabel, &args));
}
