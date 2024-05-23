// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use criticaltrust::keys::newtypes::PublicKeyBytes;
use criticaltrust::keys::{KeyAlgorithm, KeyRole, PublicKey};

fn main() {
    let whitelabel = criticalup_cli::WhitelabelConfig {
        name: "criticalup",
        http_user_agent: concat!("criticalup/", env!("CARGO_PKG_VERSION")),
        download_server_url: "https://criticalup-downloads.ferrocene.dev".into(),
        customer_portal_url: "https://customers.ferrocene.dev/".into(),
        // TODO: this key is not permanent, and must be changed before criticalup is released. The
        // key was ephemeral when it was generated, and is not persisted anywhere. If we keep it
        // as-is in the binaries we release, we won't be able to change the signing keys.
        trust_root: PublicKey {
            role: KeyRole::Root,
            algorithm: KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            expiry: None,
            public: PublicKeyBytes::borrowed(&[
                48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206, 61, 3, 1,
                7, 3, 66, 0, 4, 145, 91, 152, 186, 48, 109, 66, 242, 84, 12, 150, 220, 124, 142,
                196, 172, 189, 48, 90, 217, 123, 214, 67, 0, 139, 219, 17, 77, 185, 56, 152, 199,
                5, 110, 157, 121, 0, 229, 172, 39, 92, 217, 125, 234, 61, 139, 231, 170, 22, 176,
                174, 126, 100, 167, 20, 202, 250, 184, 237, 39, 79, 233, 75, 136,
            ]),
        },
        test_mode: false,
    };

    let args = std::env::args_os().collect::<Vec<_>>();
    std::process::exit(criticalup_cli::main(whitelabel, &args));
}
