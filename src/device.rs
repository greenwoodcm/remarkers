use anyhow::{anyhow, Result};
use ssh2::{Channel, Session};
use std::{fs::File, io::{Read, Write}, net::TcpStream, path::{Path, PathBuf}, time::{Duration, UNIX_EPOCH}};
use tracing::{debug, info};

const USB_SOURCE_USER: &str = "root";
const USB_SOURCE_HOST: &str = "10.11.99.1";

const USB_SOURCE_ROOT_PATH: &str = "/home/root/.local/share/remarkable/xochitl/";

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;

const GZIP_ENABLED: bool = false;

/// Logical representation of the Remarkable, connected via SSH
pub struct Remarkable {
    ssh_session: Session,
}

impl Remarkable {
    pub fn open() -> Result<Self> {
        let tcp = TcpStream::connect(
            format!("{USB_SOURCE_HOST}:22"),
        )?;
        let mut ssh_session = Session::new()?;
        ssh_session.set_tcp_stream(tcp);
        ssh_session.handshake()?;
        ssh_session.userauth_pubkey_file(
            USB_SOURCE_USER,
            Some(&PathBuf::from("/Users/greenwd/.ssh/id_rsa_remarkable.pub")),
            &PathBuf::from("/Users/greenwd/.ssh/id_rsa_remarkable"),
            None,
        )?;

        Ok(Self { ssh_session })
    }

    fn ssh_cmd_with_stdout<T: CmdOutput>(&self, cmd: &str) -> Result<T> {
        info!("Executing SSH cmd: {cmd}");
        let mut ssh_channel = self.ssh_session.channel_session()?;
        ssh_channel.exec(cmd)?;

        let mut output = T::default();
        output.read_from_channel(&mut ssh_channel)?;
        info!("SSH channel exit status: {:?}", ssh_channel.exit_status());

        ssh_channel.wait_close()?;

        Ok(output)
    }

    pub fn rsync_from_device_to<P: AsRef<Path>>(
        &self,
        to_local_dir: P,
    ) -> Result<()> {
        let remote_dir = PathBuf::from(USB_SOURCE_ROOT_PATH);
        let local_dir = to_local_dir.as_ref();
        info!("syncing reMarkable tablet content to local directory: {local_dir:?}");

        let ftp = self.ssh_session.sftp()?;
        let root_dir = ftp.readdir(&remote_dir)?;

        let mut created = 0;
        let mut updated = 0;
        let mut skipped = 0;
        for (path, stat) in root_dir {
            debug!("Sync evaluating {path:?}");
            let rel_path = path.strip_prefix(&remote_dir)?;
            let local_path = local_dir.join(rel_path);

            match std::fs::metadata(&local_path) {
                Ok(meta) => {
                    let remote_mod = Duration::from_secs(stat.mtime.ok_or(anyhow!(""))?);
                    let local_mod = meta
                        .modified()?
                        .duration_since(UNIX_EPOCH)?;
                    debug!("Sync encountered remote_mod={remote_mod:?}, local_mod={local_mod:?}");

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
                    if e.kind() == std::io::ErrorKind::NotFound {
                        debug!("Creating based on missing local file: {rel_path:?} to {local_path:?}");
                        let mut remote_file = ftp.open(&path)?;
                        let mut local_file = File::create(&local_path)?;
                        std::io::copy(&mut remote_file, &mut local_file)?;
                        created += 1;
                    } else {
                        return Err(e.into());
                    }
                }
            }
        }

        info!("Sync created {created} files, updated {updated} files, skipped {skipped} files");
        Ok(())
    }

    pub fn streamer(&self) -> Result<RemarkableStreamer> {
        RemarkableStreamer::new(self)
    }
}

trait CmdOutput: Default {
    fn read_from_channel(&mut self, channel: &mut Channel) -> Result<()>;
}

impl CmdOutput for String {
    fn read_from_channel(&mut self, channel: &mut Channel) -> Result<()> {
        channel.read_to_string(self)?;
        Ok(())
    }
}

impl CmdOutput for Vec<u8> {
    fn read_from_channel(&mut self, channel: &mut Channel) -> Result<()> {
        channel.read_to_end(self)?;
        Ok(())
    }
}

pub struct RemarkableStreamer<'a> {
    #[allow(unused)]
    remarkable: &'a Remarkable,
    xochitl_pid: u32,
    frame_buffer_offset: usize,
}

impl <'a> RemarkableStreamer<'a> {
    fn new(remarkable: &'a Remarkable) -> Result<Self> {
        let xochitl_pid = RemarkableStreamer::xochitl_pid(remarkable)?;
        let frame_buffer_offset = RemarkableStreamer::get_frame_buffer_offset(remarkable, xochitl_pid)?;

        Ok(Self {
            remarkable,
            xochitl_pid,
            frame_buffer_offset,
        })
    }

    fn xochitl_pid(remarkable: &Remarkable) -> Result<u32> {
        remarkable
            .ssh_cmd_with_stdout("pidof xochitl")
            .and_then(|stdout: String| stdout.trim().parse::<u32>().map_err(|e| e.into()))
    }

    fn get_frame_buffer_offset(remarkable: &Remarkable, pid: u32) -> Result<usize> {
        let cmd = format!("cat /proc/{pid}/maps");
        let stdout: String = remarkable.ssh_cmd_with_stdout(&cmd)?;

        let fb0_line = stdout
            .split('\n')
            .filter(|line| line.contains("/dev/fb0"))
            .next();
        info!("line: {fb0_line:?}");

        let addr = fb0_line.ok_or(anyhow::anyhow!("asdf"))?.split(['-', ' ']).skip(1).next();
        info!("addr: {addr:?}");

        let addr_num = usize::from_str_radix(addr.unwrap(), 16)?;
        let addr_num = addr_num + 8;
        info!("addr num: {addr_num}");
        Ok(addr_num)
    }

    pub fn frame_buffer(&self) -> Result<Vec<u8>> {
        let pid = self.xochitl_pid;

        let img_bytes = WIDTH * HEIGHT * BYTES_PER_PIXEL;
        let block_size = 4096;
        let skip_count = self.frame_buffer_offset / block_size;
        let block_count = (img_bytes - (self.frame_buffer_offset % block_size)).div_ceil(block_size);

        let dd_begin = std::time::Instant::now();
        let gzip_suffix = if GZIP_ENABLED { "| gzip" } else { "" };
        let remote_cmd = format!("dd if=/proc/{pid}/mem bs={block_size} skip={skip_count} count={block_count} {gzip_suffix}");

        let output: Vec<u8> = self.remarkable.ssh_cmd_with_stdout(&remote_cmd)?;
        info!("dd took {:?}", dd_begin.elapsed());

        if GZIP_ENABLED {
            let mut gz = flate2::write::GzDecoder::new(Vec::new());
            gz.write_all(&output[..])?;

            let decomped = gz.finish().unwrap();

            info!("Decompressed GZIP data from {} bytes to {} bytes", output.len(), decomped.len());
            Ok(decomped)
        } else {
            Ok(output)
        }
    }
}