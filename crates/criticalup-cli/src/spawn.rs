use crate::errors::Error;
use std::process::Command;

/// Utility function to spawn a process and have it replace the current one.
///
/// This is for Unix based systems. For Windows based systems, please see the function below.
#[cfg(unix)]
pub(crate) fn spawn_command(mut command: Command) -> Result<(), Error> {
    use std::os::unix::process::CommandExt;
    use std::path::PathBuf;

    let path = PathBuf::from(command.get_program());

    // exec() replaces the current process with the process we're about to invoke. Thus if it
    // returns at all it means the invocation failed.
    Err(Error::FailedToInvokeProxiedCommand(path, command.exec()))
}

/// Utility function to spawn a child process. for Windows based systems.
///
/// Windows does not have an `exevcp` equivalent.
///
/// We **cannot** replace our current process.
///
/// Instead, we use the strategy `cargo` and `rustup` use:
/// https://github.com/rust-lang/cargo/blob/403fbe2b490d6cbb715ed768462bb7f977a6d514/crates/cargo-util/src/process_builder.rs#L609-L626
/// https://github.com/rust-lang/rustup/blob/a7c0c45b2daaa149ac9a8e14a7270c855cd2b334/src/command.rs#L37-L56
#[cfg(windows)]
pub(crate) fn spawn_command(mut command: Command) -> Result<(), Error> {
    use std::path::PathBuf;
    use windows_sys::Win32::Foundation::{BOOL, FALSE, TRUE};
    use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;

    unsafe extern "system" fn ctrlc_handler(_: u32) -> BOOL {
        // Do nothing; let the child process handle it.
        TRUE
    }

    unsafe {
        if SetConsoleCtrlHandler(Some(ctrlc_handler), TRUE) == FALSE {
            return Err(Error::CtrlHandler);
        }
    }

    let path = PathBuf::from(command.get_program());

    // Success or failure is irrelevant, we simply want to run the task then exit.
    let exit = command
        .status()
        .map_err(|e| Error::FailedToInvokeProxiedCommand(path, e))?;
    std::process::exit(exit.code().unwrap_or(1));
}
