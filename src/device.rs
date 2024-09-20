use anyhow::{anyhow, Result};
use std::{io::Write, path::Path, process::Command};
use tracing::info;

use crate::command::exec_cmd_with_stdio;

const USB_SOURCE_USER: &str = "root";
const USB_SOURCE_HOST: &str = "10.11.99.1";

macro_rules! cmd {
    ($command_and_args:literal) => {{
        let command_and_args: String = format!($command_and_args);
        let mut elems = command_and_args.split(" ");

        let program = elems.next().expect("asdf");
        let mut cmd = std::process::Command::new(program);
        for elem in elems {
            cmd.arg(elem);
        }
        cmd
    }};
}

const WIDTH: usize = 1872;
const HEIGHT: usize = 1404;
const BYTES_PER_PIXEL: usize = 2;

const GZIP_ENABLED: bool = false;

/// Logical representation of the Remarkable, connected via SSH
#[derive(Default)]
pub struct Remarkable {}

impl Remarkable {
    fn cmd_to_ssh_cmd(&self, cmd: &mut Command) -> Command {
        let mut remote_cmd = vec![format!(
            "{}",
            cmd.get_program().to_str().unwrap().to_string()
        )];
        for arg in cmd.get_args() {
            remote_cmd.push(arg.to_str().unwrap().to_string());
        }
        let remote_cmd = remote_cmd.join(" ");

        cmd!("ssh {USB_SOURCE_USER}@{USB_SOURCE_HOST} {remote_cmd}")
    }

    fn ssh_cmd_with_stdio(&self, cmd: &mut Command) -> Result<(String, String)> {
        let mut ssh_cmd = self.cmd_to_ssh_cmd(cmd);
        exec_cmd_with_stdio(&mut ssh_cmd)
    }

    pub fn rsync_from<P1: AsRef<Path>, P2: AsRef<Path>>(
        &self,
        from_remote_dir: P1,
        to_local_dir: P2,
    ) -> Result<()> {
        let remote_dir = from_remote_dir.as_ref();
        let local_dir = to_local_dir.as_ref();

        let remote_dir_str = path_to_str(remote_dir)?;
        let local_dir_str = path_to_str(local_dir)?;

        let mut cmd = cmd!("rsync --recursive {USB_SOURCE_USER}@{USB_SOURCE_HOST}:{remote_dir_str} {local_dir_str}");

        exec_cmd_with_stdio(&mut cmd).map(|_output| ())
    }

    pub fn streamer(&self) -> Result<RemarkableStreamer> {
        RemarkableStreamer::new(self)
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
        let mut cmd = cmd!("pidof xochitl");

        remarkable
            .ssh_cmd_with_stdio(&mut cmd)
            .and_then(|(stdout, _stderr)| stdout.trim().parse::<u32>().map_err(|e| e.into()))
    }

    fn get_frame_buffer_offset(remarkable: &Remarkable, pid: u32) -> Result<usize> {
        let mut cmd = cmd!("cat /proc/{pid}/maps");

        let (stdout, _stderr) = remarkable.ssh_cmd_with_stdio(&mut cmd)?;

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
        let mut cmd = Command::new("ssh");
        cmd.arg(format!("{USB_SOURCE_USER}@{USB_SOURCE_HOST}"));
        cmd.arg(remote_cmd);
        info!("Executing command: {cmd:?}");

        let output = cmd.output()?;
        info!("dd took {:?}", dd_begin.elapsed());

        if output.status.success() {
            if GZIP_ENABLED {
                let mut gz = flate2::write::GzDecoder::new(Vec::new());
                gz.write_all(&output.stdout[..])?;

                let decomped = gz.finish().unwrap();

                info!("Decompressed GZIP data from {} bytes to {} bytes", output.stdout.len(), decomped.len());
                info!("results equal?: {}", decomped == &output.stdout[..]);
                info!("buf sum: {:?}", decomped.iter().cloned().reduce(|a: u8, b| a.wrapping_add(b)));
                Ok(decomped)
            } else {
                Ok(output.stdout)
            }
        } else {
            Err(anyhow!(
                "failed command [{:?}] with exit code {}: {}",
                cmd, output.status, std::str::from_utf8(&output.stderr)?.trim()))
        }
    }
}

fn path_to_str(p: &Path) -> Result<&str> {
    p.to_str().ok_or(anyhow!("failed to stringify path: {p:?}"))
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_cmd_macro() {
        let cmd = cmd!("abc");
        let actual = unpack_command(&cmd);

        assert_eq!("abc", actual.0);
        assert_eq!(Vec::<&str>::new(), actual.1);

        let cmd = cmd!("abc def");
        let actual = unpack_command(&cmd);

        assert_eq!("abc", actual.0);
        assert_eq!(vec!["def"], actual.1);

        let lit = 12345;
        let cmd = cmd!("abc arg_{lit}");
        let actual = unpack_command(&cmd);

        assert_eq!("abc", actual.0);
        assert_eq!(vec!["arg_12345"], actual.1);
    }

    fn unpack_command<'a>(cmd: &'a Command) -> (&'a str, Vec<&'a str>) {
        let program = cmd.get_program().to_str().unwrap();
        let args: Vec<_> = cmd
            .get_args()
            .into_iter()
            .map(|a| a.to_str().unwrap())
            .collect();
        (program, args)
    }
}
