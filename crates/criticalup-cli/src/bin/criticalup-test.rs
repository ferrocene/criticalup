// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Variant of the criticalup binary with mocking support, used by the criticalup test suite to
//! perform tests without connecting to the production download servers.

#[tokio::main]
async fn main() {
    if std::env::var_os("CRITICALUP_TESTING_IN_PROGRESS").is_none() {
        panic!("This is an internal test tool. Do not run manually.");
    }

    let whitelabel = criticalup_cli::WhitelabelConfig {
        name: "criticalup-test",
        http_user_agent: concat!("criticalup-test/", env!("CARGO_PKG_VERSION")),
        download_server_url: std::env::var("CRITICALUP_TEST_DOWNLOAD_SERVER_URL")
            .expect("missing CRITICALUP_TEST_DOWNLOAD_SERVER_URL"),
        customer_portal_url: std::env::var("CRITICALUP_TEST_CUSTOMER_PORTAL_URL")
            .expect("missing CRITICALUP_TEST_CUSTOMER_PORTAL_URL"),
        trust_root: serde_json::from_str(
            &std::env::var("CRITICALUP_TEST_TRUST_ROOT")
                .expect("missing CRITICALUP_TEST_TRUST_ROOT"),
        )
        .expect("CRITICALUP_TEST_TRUST_ROOT should be a valid JSON encoded PublicKey object"),
        test_mode: true,
    };

    let args = std::env::args_os().collect::<Vec<_>>();
    std::process::exit(criticalup_cli::main(whitelabel, &args).await);
}
