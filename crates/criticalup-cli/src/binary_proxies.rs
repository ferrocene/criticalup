// SPDX-FileCopyrightText: The Ferrocene Developers
// SPDX-License-Identifier: MIT OR Apache-2.0

use crate::errors::Error;
use crate::spawn;
use criticalup_core::config::{Config, WhitelabelConfig};
use criticalup_core::project_manifest::ProjectManifest;
use criticalup_core::state::State;
use std::env::JoinPathsError;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(crate) fn proxy(whitelabel: WhitelabelConfig) -> Result<(), Error> {
    let binary_name = arg0(&whitelabel)?;
    let args: Vec<_> = std::env::args_os().skip(1).collect();

    let config = Config::detect(whitelabel)?;
    let state = State::load(&config)?;

    let manifest_path = ProjectManifest::discover_canonical_path(
        std::env::var_os("CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH")
            .map(std::path::PathBuf::from)
            .as_deref(),
    )?;

    let project_manifest = ProjectManifest::load(manifest_path.as_path())?;

    let Some((installation_id, resolved_path)) = project_manifest
        .products()
        .iter()
        .map(|p| p.installation_id())
        .filter_map(|id| {
            state
                .resolve_binary_proxy(&id, &binary_name)
                .map(|p| (id, p))
        })
        .next()
    else {
        return Err(Error::BinaryNotInstalled(binary_name));
    };

    let mut command = Command::new(
        config
            .paths
            .installation_dir
            .join(installation_id.clone())
            .join(resolved_path),
    );

    // In order to ensure, for example, our `cargo` invokes our `rustc` we
    // append the proxy dir to the path.
    //
    // For some particularly niche use cases, users may find themselves wanting
    // to override the `rustc` called, and they may want to do that by setting
    // `PATH` themselves, but they:
    // 1) Shouldn't do that, and
    // 2) Can set `RUSTC` which `cargo` already supports.
    let additional_bin_path = config.paths.proxies_dir.clone();
    // We need to also set the library path according to
    // https://doc.rust-lang.org/cargo/reference/environment-variables.html#dynamic-library-paths
    // Notably: On Windows this is the same as the binary path.
    let additional_lib_path = config
        .paths
        .installation_dir
        .clone()
        .join(installation_id)
        .join("lib");

    #[cfg(target_os = "macos")]
    prepend_path_to_var_for_command(
        &mut command,
        "DYLD_FALLBACK_LIBRARY_PATH",
        vec![additional_lib_path],
    )?;
    #[cfg(target_os = "linux")]
    prepend_path_to_var_for_command(&mut command, "LD_LIBRARY_PATH", vec![additional_lib_path])?;
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    prepend_path_to_var_for_command(&mut command, "PATH", vec![additional_bin_path])?;

    #[cfg(target_os = "windows")]
    prepend_path_to_var_for_command(
        &mut command,
        "PATH",
        vec![additional_bin_path, additional_lib_path],
    )?;

    // CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH is an environment variable set by CriticalUp
    // to make sure that the canonical manifest path is available to CriticalUp when using cargo
    // with project with dependencies.
    //
    // This is required because cargo changes the current directory to the project dependency
    // location. The repercussion is that the criticalup.toml manifest will not be found if the user
    // runs cargo commands due to this directory switching.
    //
    // Important: Users must never set this on their own!
    command
        .env(
            "CRITICALUP_CURRENT_PROJ_MANIFEST_CANONICAL_PATH",
            manifest_path,
        )
        .args(args)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    spawn::spawn_command(command)
}

pub(crate) fn arg0(whitelabel: &WhitelabelConfig) -> Result<String, Error> {
    let mut arg0 = std::env::args_os()
        .next()
        .expect("missing arg0, should never happen");

    // Helper to allow us to simulate binary proxies in the test suite without having to copy or
    // symlink files around. Due to being gated under test_mode it will not be available in
    // production binaries.
    if whitelabel.test_mode {
        if let Some(overridden) = std::env::var_os("CRITICALUP_TEST_OVERRIDE_ARG0") {
            arg0 = overridden;
        }
    }

    let arg0 = Path::new(&arg0);
    arg0.file_name()
        .unwrap_or(arg0.as_os_str())
        .to_str()
        .ok_or(Error::NonUtf8Arg0)
        .map(|s| s.to_string())
}

fn prepend_path_to_var_for_command(
    command: &mut Command,
    env_var: &str,
    new: Vec<PathBuf>,
) -> Result<(), JoinPathsError> {
    let mut existing_vals = if let Some(existing_vals) = std::env::var_os(env_var) {
        std::env::split_paths(&existing_vals).collect::<Vec<_>>()
    } else {
        vec![]
    };
    let mut updated_val = new;
    updated_val.append(&mut existing_vals);
    command.env(env_var, std::env::join_paths(updated_val)?);
    Ok(())
}
