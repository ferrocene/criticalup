use criticaltrust::keys::newtypes::PublicKeyBytes;
use criticaltrust::keys::{KeyAlgorithm, KeyRole, PublicKey};

fn main() {
    let whitelabel = criticalup_cli::WhitelabelConfig {
        name: "criticalup-dev",
        http_user_agent: concat!("criticalup/", env!("CARGO_PKG_VERSION"), " (dev)"),
        download_server_url: "https://criticalup-downloads-dev.ferrocene.dev".into(),
        customer_portal_url: "https://customers-dev.ferrocene.dev".into(),
        // TODO: this key is not permanent, and must be changed before criticalup is released. The
        // key was ephemeral when it was generated, and is not persisted anywhere. If we keep it
        // as-is in the binaries we release, we won't be able to change the signing keys.
        trust_root: PublicKey {
            role: KeyRole::Root,
            algorithm: KeyAlgorithm::EcdsaP256Sha256Asn1SpkiDer,
            expiry: None,
            public: PublicKeyBytes::borrowed(&[
                48, 89, 48, 19, 6, 7, 42, 134, 72, 206, 61, 2, 1, 6, 8, 42, 134, 72, 206, 61, 3, 1,
                7, 3, 66, 0, 4, 19, 173, 118, 198, 129, 248, 105, 3, 11, 48, 104, 0, 121, 174, 246,
                253, 35, 160, 246, 160, 6, 104, 28, 0, 105, 25, 55, 112, 246, 234, 57, 192, 254,
                247, 238, 41, 63, 104, 251, 171, 202, 168, 117, 89, 203, 124, 0, 92, 203, 94, 171,
                68, 232, 71, 66, 59, 100, 64, 66, 53, 107, 204, 134, 227,
            ]),
        },
        test_mode: false,
    };

    let args = std::env::args_os().collect::<Vec<_>>();
    std::process::exit(criticalup_cli::main(whitelabel, &args));
}
