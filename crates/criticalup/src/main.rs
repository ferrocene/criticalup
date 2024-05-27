// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::keys::newtypes::PublicKeyBytes;
use criticaltrust::keys::{KeyAlgorithm, KeyRole, PublicKey};

fn main() {
    let whitelabel = criticalup_cli::WhitelabelConfig {
        name: "criticalup",
        http_user_agent: concat!("criticalup/", env!("CARGO_PKG_VERSION")),
        download_server_url: "https://criticalup-downloads.ferrocene.dev".into(),
        customer_portal_url: "https://customers.ferrocene.dev".into(),
        // TODO: this key is not permanent, and must be changed before criticalup is released. The
        // key was ephemeral when it was generated, and is not persisted anywhere. If we keep it
        // as-is in the binaries we release, we won't be able to change the signing keys.
        trust_root: PublicKey {
            role: KeyRole::Root,
            algorithm: KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            expiry: None,
            public: PublicKeyBytes::borrowed(&[
                48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206, 61, 3, 1,
                7, 3, 66, 0, 4, 68, 168, 159, 251, 220, 122, 178, 116, 99, 232, 131, 60, 141, 227,
                61, 4, 83, 99, 18, 5, 142, 157, 245, 214, 142, 145, 36, 8, 168, 234, 188, 23, 236,
                178, 151, 94, 119, 120, 150, 188, 22, 232, 0, 94, 59, 232, 134, 165, 12, 158, 7,
                112, 156, 114, 150, 137, 52, 193, 254, 180, 81, 68, 158, 255,
            ]),
        },
        test_mode: false,
    };

    let args = std::env::args_os().collect::<Vec<_>>();
    std::process::exit(criticalup_cli::main(whitelabel, &args));
}
