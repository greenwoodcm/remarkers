use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tracing::{info, trace};

const SOURCE_PATH: &str = "/home/root/.local/share/remarkable/xochitl/";

const USB_SOURCE_USER: &str = "root";
const USB_SOURCE_HOST: &str = "10.11.99.1";

pub fn sync_remarkable_to_dir<P: AsRef<Path>>(local_dir: P) -> Result<()> {
    info!(
        "syncing reMarkable tablet content to local directory: {:?}",
        local_dir.as_ref()
    );

    let result = Command::new("rsync")
        .arg("--recursive")
        .arg(format!("{USB_SOURCE_USER}@{USB_SOURCE_HOST}:{SOURCE_PATH}"))
        .arg(local_dir.as_ref())
        .output()?;

    trace!(
        "output of rsync: {}\nSTDOUT: {}\nSTDERR: {}",
        result.status,
        std::str::from_utf8(&result.stdout)?,
        std::str::from_utf8(&result.stderr)?,
    );

    Ok(())
}
