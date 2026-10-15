//! Forward `cargo demo` to the existing development script.
use std::{io, path::Path, process::Command};

fn main() -> io::Result<()> {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let mut command = Command::new("node");
    command
        .current_dir(repository)
        .arg("scripts/demo.mjs")
        .args(std::env::args_os().skip(1));

    // Replace the wrapper so signals reach Node's child-process cleanup handlers.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        Err(command.exec())
    }
    #[cfg(not(unix))]
    {
        std::process::exit(command.status()?.code().unwrap_or(1));
    }
}
