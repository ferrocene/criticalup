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
                7, 3, 66, 0, 4, 145, 199, 131, 120, 202, 45, 142, 29, 104, 51, 133, 141, 86, 87,
                31, 25, 63, 99, 132, 215, 24, 171, 63, 51, 54, 72, 153, 241, 61, 193, 107, 196,
                195, 226, 200, 57, 245, 120, 201, 209, 158, 75, 216, 115, 53, 114, 11, 12, 108,
                186, 206, 173, 85, 153, 172, 172, 172, 191, 74, 241, 22, 96, 62, 242,
            ]),
        },
        test_mode: false,
    };

    let args = std::env::args_os().collect::<Vec<_>>();
    std::process::exit(criticalup_cli::main(whitelabel, &args));
}
