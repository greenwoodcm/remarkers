use anyhow::{anyhow, Result};
use std::{path::Path, process::Command};
use tracing::{info, trace};

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

/// Logical representation of the Remarkable, connected via SSH
#[derive(Default)]
pub struct Remarkable {}

impl Remarkable {
    fn do_cmd(&self, cmd: &mut Command) -> Result<(String, String)> {
        trace!("executing cmd: {:?}", &cmd);

        let result = cmd.output()?;
        let stdout = std::str::from_utf8(&result.stdout)?;
        let stderr = std::str::from_utf8(&result.stderr)?;

        trace!(
            "output of {:?}: {}\nSTDOUT: {}\nSTDERR: {}",
            cmd.get_program(),
            result.status,
            std::str::from_utf8(&result.stdout)?,
            std::str::from_utf8(&result.stderr)?,
        );

        if result.status.success() {
            Ok((stdout.to_string(), stderr.to_string()))
        } else {
            Err(anyhow!(
                "failed command {:?} with exit code {}",
                cmd.get_program(),
                result.status
            ))
        }
    }

    fn ssh_cmd(&self, cmd: &mut Command) -> Result<(String, String)> {
        let mut remote_cmd = vec![format!(
            "{}",
            cmd.get_program().to_str().unwrap().to_string()
        )];
        for arg in cmd.get_args() {
            remote_cmd.push(arg.to_str().unwrap().to_string());
        }
        let remote_cmd = remote_cmd.join(" ");

        let mut ssh_cmd = cmd!("ssh {USB_SOURCE_USER}@{USB_SOURCE_HOST} {remote_cmd}");
        self.do_cmd(&mut ssh_cmd)
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

        self.do_cmd(&mut cmd).map(|_output| ())
    }

    pub fn ls(&self) -> Result<(String, String)> {
        let mut cmd = cmd!("ls");
        self.ssh_cmd(&mut cmd)
    }

    pub fn xochitl_pid(&self) -> Result<u32> {
        let mut cmd = cmd!("pidof xochitl");

        self.ssh_cmd(&mut cmd)
            .and_then(|(stdout, _stderr)| stdout.trim().parse::<u32>().map_err(|e| e.into()))
    }

    pub fn get_map(&self) -> Result<String> {
        let pid = self.xochitl_pid()?;
        let mut cmd = cmd!("cat /proc/{pid}/maps");

        let (stdout, _stderr) = self.ssh_cmd(&mut cmd)?;
        info!("{}", &stdout);

        let fb0_line = stdout
            .split('\n')
            .filter(|line| line.contains("/dev/fb0"))
            .next();
        info!("line: {fb0_line:?}");

        let addr = fb0_line.ok_or(anyhow::anyhow!("asdf"))?.split('-').next();
        info!("addr: {addr:?}");

        let addr_num = usize::from_str_radix(addr.unwrap(), 16)?;
        let addr_num = addr_num + 8;
        info!("addr num: {addr_num}");
        Ok(stdout)
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
