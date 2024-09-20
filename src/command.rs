use anyhow::{anyhow, Result};
use std::process::{Command, Output};
use tracing::trace;

pub fn exec_cmd(cmd: &mut Command) -> Result<Output> {
    trace!("executing cmd: {:?}", &cmd);

    let result = cmd.output()?;

    trace!(
        "output of {:?}: {}\nSTDOUT: {}\nSTDERR: {}",
        cmd.get_program(),
        result.status,
        std::str::from_utf8(&result.stdout)?,
        std::str::from_utf8(&result.stderr)?,
    );

    if result.status.success() {
        Ok(result)
    } else {
        Err(anyhow!(
            "failed command {:?} with exit code {}",
            cmd.get_program(),
            result.status
        ))
    }
}

pub fn exec_cmd_with_stdio(cmd: &mut Command) -> Result<(String, String)> {
    let result = exec_cmd(cmd)?;

    let stdout = std::str::from_utf8(&result.stdout)?;
    let stderr = std::str::from_utf8(&result.stderr)?;

    Ok((stdout.to_string(), stderr.to_string()))
}