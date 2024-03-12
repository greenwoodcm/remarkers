use anyhow::Result;
use std::path::Path;
use tracing::info;

const SOURCE_PATH: &str = "/home/root/.local/share/remarkable/xochitl/";

pub fn sync_remarkable_to_dir<P: AsRef<Path>>(local_dir: P) -> Result<()> {
    info!(
        "syncing reMarkable tablet content to local directory: {:?}",
        local_dir.as_ref()
    );

    let rem = crate::device::Remarkable::default();
    rem.rsync_from(SOURCE_PATH, local_dir)
}
