use anyhow::{anyhow, Context, Result};
use ssh2::{Channel, Session};
use std::{
    fs::File,
    io::{Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::Mutex,
    time::{Duration, UNIX_EPOCH},
};
use tracing::{debug, info, trace, warn};

const USB_SOURCE_USER: &str = "root";
const USB_SOURCE_HOST: &str = "10.11.99.1";

const USB_SOURCE_ROOT_PATH: &str = "/home/root/.local/share/remarkable/xochitl/";

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;

const METADATA_COMMAND_TIMEOUT: Duration = Duration::from_millis(500);
const FRAME_COMMAND_TIMEOUT: Duration = Duration::from_secs(5);

/// Whether to GZIP compress frame data on the device and then decompress
/// the data client side after the transfer.
const GZIP_ENABLED: bool = false;

/// Whether to use dynamically linked libssh2 for the remote `dd` command
/// or shell out to the `ssh` binary.
///
/// Performance testing of these two flags suggests that doing neither
/// is the best option:
///
/// gzip  ssh2
/// false false = 440-540 ms per frame
/// false true  = 660-720 ms per frame
/// true  false = 680-700 ms per frame
/// true  true  = 520-550 ms per frame
const SSH2_ENABLED: bool = false;

/// Logical representation of the Remarkable, connected via SSH
pub struct Remarkable {
    ssh_session: Session,
}

impl Remarkable {
    pub fn open() -> Result<Self> {
        trace!("Connecting to Remarkable at {USB_SOURCE_HOST}:22");
        let tcp = TcpStream::connect(format!("{USB_SOURCE_HOST}:22"))?;
        trace!("Established TCP connection to Remarkable");
        let mut ssh_session = Session::new()?;
        ssh_session.set_tcp_stream(tcp);
        ssh_session.handshake()?;
        ssh_session.userauth_pubkey_file(
            USB_SOURCE_USER,
            Some(&PathBuf::from("/Users/greenwd/.ssh/id_rsa_remarkable.pub")),
            &PathBuf::from("/Users/greenwd/.ssh/id_rsa_remarkable"),
            None,
        )?;

        trace!("Connected to Remarkable at {USB_SOURCE_HOST}:22");
        Ok(Self { ssh_session })
    }

    async fn ssh_cmd_with_stdout<T: CmdOutput>(&self, cmd: &str, timeout: Duration) -> Result<T> {
        debug!("Executing SSH cmd: {cmd}");

        let start = std::time::Instant::now();

        if SSH2_ENABLED {
            let mut ssh_channel = self.ssh_session.channel_session()?;
            debug!("SSH channel opened in {:?}", start.elapsed());
            ssh_channel.exec(cmd)?;
            debug!("SSH channel executed in {:?}", start.elapsed());

            let output = T::read_from_channel(&mut ssh_channel)?;
            debug!("SSH channel read-from in {:?}", start.elapsed());
            debug!("SSH channel exit status: {:?}", ssh_channel.exit_status());

            ssh_channel.wait_close()?;
            debug!("SSH channel closed in {:?}", start.elapsed());
            Ok(output)
        } else {
            let cmd = tokio::process::Command::new("ssh")
                .arg(format!("{USB_SOURCE_USER}@{USB_SOURCE_HOST}"))
                .arg(cmd)
                .output();

            let output = tokio::time::timeout(timeout, cmd)
                .await
                .context("timed out waiting for SSH command")?
                .context("error executing SSH command")?;
            debug!("SSH executed in {:?}", start.elapsed());
            Ok(T::from_vec(output.stdout))
        }
    }

    pub fn rsync_from_device_to<P: AsRef<Path>>(&self, to_local_dir: P) -> Result<()> {
        self.rsync_from_device_dir_to(USB_SOURCE_ROOT_PATH, to_local_dir)
            .map(|_stats| ())
    }

    fn rsync_from_device_dir_to<P0: AsRef<Path>, P1: AsRef<Path>>(
        &self,
        from_device_dir: P0,
        to_local_dir: P1,
    ) -> Result<(u32, u32, u32)> {
        let remote_dir = from_device_dir.as_ref();
        let local_dir = to_local_dir.as_ref();
        info!("syncing reMarkable tablet content to local directory: {local_dir:?}");
        std::fs::create_dir_all(local_dir)?;

        let ftp = self.ssh_session.sftp()?;
        let root_dir = ftp.readdir(&remote_dir)?;

        let mut created = 0;
        let mut updated = 0;
        let mut skipped = 0;
        for (path, stat) in root_dir {
            debug!("Sync evaluating {path:?}");
            let rel_path = path.strip_prefix(&remote_dir)?;
            let local_path = local_dir.join(rel_path);

            if stat.is_dir() {
                debug!("Traversing remote directory {path:?}");
                let (inner_created, inner_updated, inner_skipped) =
                    self.rsync_from_device_dir_to(&path, local_path)?;
                created += inner_created;
                updated += inner_updated;
                skipped += inner_skipped;
            } else {
                debug!("Encountered file, checking local filesystem for {local_path:?}");
                match std::fs::metadata(&local_path) {
                    Ok(meta) => {
                        let remote_mod = Duration::from_secs(stat.mtime.ok_or(anyhow!(""))?);
                        let local_mod = meta.modified()?.duration_since(UNIX_EPOCH)?;
                        debug!(
                            "Sync encountered remote_mod={remote_mod:?}, local_mod={local_mod:?}"
                        );

                        if remote_mod > local_mod {
                            debug!("Syncing based on newer mtime: {rel_path:?} to {local_path:?}");
                            let mut remote_file = ftp.open(&path)?;
                            let mut local_file = File::create(&local_path)?;
                            std::io::copy(&mut remote_file, &mut local_file)?;
                            updated += 1;
                        } else {
                            debug!("Syncing based on older mtime: {rel_path:?} to {local_path:?}");
                            skipped += 1;
                        }
                    }
                    Err(e) => {
                        debug!("Error fetching file metadata: {e:?}");
                        if e.kind() == std::io::ErrorKind::NotFound {
                            debug!(
                                "Creating based on missing local file: {rel_path:?} to {local_path:?}"
                            );
                            let mut remote_file = ftp.open(&path)?;
                            let mut local_file = File::create(&local_path)?;
                            std::io::copy(&mut remote_file, &mut local_file).map_err(|e| {
                                debug!("Error copying from remote to local: {e:?}");
                                anyhow!("Error copying from remote to local: {e:?}")
                            })?;
                            created += 1;
                        } else {
                            return Err(e.into());
                        }
                    }
                }
            }
        }

        info!("Sync created {created} files, updated {updated} files, skipped {skipped} files");
        Ok((created, updated, skipped))
    }

    pub async fn streamer(&self) -> Result<RemarkableStreamer> {
        RemarkableStreamer::new(self).await
    }
}

trait CmdOutput: Default {
    fn from_vec(vec: Vec<u8>) -> Self;
    fn read_from_channel(channel: &mut Channel) -> Result<Self>;
}

impl CmdOutput for String {
    fn from_vec(vec: Vec<u8>) -> Self {
        String::from_utf8(vec).unwrap()
    }

    fn read_from_channel(channel: &mut Channel) -> Result<Self> {
        let mut s = String::new();
        channel.read_to_string(&mut s)?;
        Ok(s)
    }
}

impl CmdOutput for Vec<u8> {
    fn read_from_channel(channel: &mut Channel) -> Result<Self> {
        let mut vec = Vec::with_capacity(WIDTH * HEIGHT * BYTES_PER_PIXEL);
        channel.read_to_end(&mut vec)?;
        Ok(vec)
    }

    fn from_vec(vec: Vec<u8>) -> Self {
        vec
    }
}

pub struct RemarkableStreamer<'a> {
    #[allow(unused)]
    remarkable: &'a Remarkable,
    stream_info: Mutex<RemarkableStreamInfo>,
}

struct RemarkableStreamInfo {
    xochitl_pid: u32,
    frame_buffer_offset: usize,
}

impl<'a> RemarkableStreamer<'a> {
    async fn new(remarkable: &'a Remarkable) -> Result<Self> {
        Ok(Self {
            remarkable,
            stream_info: Mutex::new(RemarkableStreamer::stream_info(remarkable).await?),
        })
    }

    async fn stream_info(remarkable: &Remarkable) -> Result<RemarkableStreamInfo> {
        let xochitl_pid = RemarkableStreamer::xochitl_pid(remarkable).await?;
        let frame_buffer_offset =
            RemarkableStreamer::get_frame_buffer_offset(remarkable, xochitl_pid).await?;
        Ok(RemarkableStreamInfo {
            xochitl_pid,
            frame_buffer_offset,
        })
    }

    async fn xochitl_pid(remarkable: &Remarkable) -> Result<u32> {
        remarkable
            .ssh_cmd_with_stdout("pidof xochitl", METADATA_COMMAND_TIMEOUT)
            .await
            .and_then(|stdout: String| stdout.trim().parse::<u32>().map_err(|e| e.into()))
    }

    async fn get_frame_buffer_offset(remarkable: &Remarkable, pid: u32) -> Result<usize> {
        let cmd = format!("cat /proc/{pid}/maps");
        let stdout: String = remarkable
            .ssh_cmd_with_stdout(&cmd, METADATA_COMMAND_TIMEOUT)
            .await?;

        let fb0_line = stdout
            .split('\n')
            .filter(|line| line.contains("/dev/fb0"))
            .next();
        debug!("line containing /dev/fb0: {fb0_line:?}");

        let addr = fb0_line
            .ok_or(anyhow!("failed to find /dev/fb0 in /proc/{pid}/maps"))?
            .split(['-', ' '])
            .skip(1)
            .next()
            .ok_or(anyhow!(
                "failed to find frame buffer offset in [{fb0_line:?}]"
            ))?;
        debug!("frame buffer offset string: {addr:?}");

        let addr_num = usize::from_str_radix(addr, 16)?;
        let addr_num = addr_num + 8;
        debug!("frame buffer offset: {addr_num}");

        Ok(addr_num)
    }

    pub async fn frame_buffer(&self) -> Result<Vec<u8>> {
        let (pid, frame_buffer_offset) = {
            let stream_info = self.stream_info.lock().expect("failed to lock stream info");
            (stream_info.xochitl_pid, stream_info.frame_buffer_offset)
        };

        let img_bytes = WIDTH * HEIGHT * BYTES_PER_PIXEL;
        let block_size = 4096;
        let skip_count = frame_buffer_offset / block_size;
        let block_count = (img_bytes - (frame_buffer_offset % block_size)).div_ceil(block_size);

        let dd_begin = std::time::Instant::now();
        let gzip_suffix = if GZIP_ENABLED { "| gzip" } else { "" };
        let remote_cmd = format!("dd if=/proc/{pid}/mem bs={block_size} skip={skip_count} count={block_count} {gzip_suffix}");

        let result: Result<Vec<u8>, _> = self
            .remarkable
            .ssh_cmd_with_stdout(&remote_cmd, FRAME_COMMAND_TIMEOUT)
            .await;
        debug!(
            "dd completed with success {} in {:?}",
            result.is_ok(),
            dd_begin.elapsed()
        );

        match result {
            Ok(output) => {
                if GZIP_ENABLED {
                    let mut gz = flate2::write::GzDecoder::new(Vec::new());
                    gz.write_all(&output[..])?;

                    let decomped = gz.finish().unwrap();

                    debug!(
                        "Decompressed GZIP data from {} bytes to {} bytes",
                        output.len(),
                        decomped.len()
                    );
                    Ok(decomped)
                } else {
                    Ok(output)
                }
            }
            Err(e) => {
                warn!("Error issuing dd to Remarkable: {e:?}");

                // in the case of a failure to complete the dd command
                // we want to refresh the pid and frame buffer offset
                // just in case they've changed
                let mut cached_stream_info =
                    self.stream_info.lock().expect("failed to lock stream info");
                *cached_stream_info = RemarkableStreamer::stream_info(&self.remarkable).await?;

                Err(e)
            }
        }
    }
}
