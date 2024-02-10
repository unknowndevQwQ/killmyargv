use base64::alphabet::STANDARD;
use base64::engine::GeneralPurpose;
use base64::Engine;
use sysinfo::{Pid, ProcessRefreshKind};
use test_binary::build_test_binary;

pub fn get_set_cmdline_path() -> Result<OsString> {
    Ok(build_test_binary("set_cmdline_from_stdin", "testbin")?)
}

pub fn set_cmdline(
    inputs: impl IntoIterator<Item = impl AsRef<str>>,
    results: impl IntoIterator<Item = impl IntoIterator<Item = impl AsRef<str>>>,
) -> Result<()> {
    let set_cmdline_path = get_set_cmdline_path()?;
    let mut set_cmdline = std::process::Command::new(set_cmdline_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let child_stdin = Option::take(&mut set_cmdline.stdin).unwrap();
    let child_stdout = Option::take(&mut set_cmdline.stdout).unwrap();
    let child_pid = set_cmdline.id();

    set_cmdline_with_child(inputs, results, child_stdin, child_stdout, child_pid)
}

pub fn set_cmdline_with_child(
    inputs: impl IntoIterator<Item = impl AsRef<str>>,
    results: impl IntoIterator<Item = impl IntoIterator<Item = impl AsRef<str>>>,
    mut child_stdin: ChildStdin,
    child_stdout: ChildStdout,
    child_pid: u32,
) -> Result<()> {
    let mut child_stdout = linereader::LineReader::new(child_stdout);
    let pid = Pid::from_u32(child_pid);
    let engine = GeneralPurpose::new(&STANDARD, Default::default());
    let mut system = sysinfo::System::new();

    for (case_str, results) in inputs.into_iter().zip(results) {
        let case_base64 = engine
            .encode(case_str.as_ref())
            .bytes()
            .filter(|ch| ch != &b'\n')
            .collect::<Vec<_>>();
        child_stdin.write(&case_base64)?;
        child_stdin.write(&[b'\n'])?;
        child_stdin.flush()?;

        if let Some(line) = child_stdout.next_line() {
            line?;
        }

        assert!(system.refresh_process_specifics(
            pid,
            ProcessRefreshKind::new().with_cmd(sysinfo::UpdateKind::Always)
        ));
        let set_cmdline_process = system.process(pid).expect("set-cmdline exits unexpectedly");
        let actual_cmdline = set_cmdline_process.cmd();
        let expected_cmdline = results
            .into_iter()
            .map(|s| s.as_ref().to_owned())
            .collect::<Vec<String>>();
        assert_eq!(actual_cmdline, &expected_cmdline);
    }
    Ok(())
}

use std::{
    error::Error,
    ffi::OsString,
    io::Write,
    process::{ChildStdin, ChildStdout, Stdio},
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
