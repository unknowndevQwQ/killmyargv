use std::error::Error;
use std::io::Write;
use std::process::Stdio;

use base64::alphabet::STANDARD;
use base64::engine::GeneralPurpose;
use base64::Engine;
use sysinfo::{Pid, ProcessRefreshKind};
use test_binary::build_test_binary;

#[test]
fn test_set_cmdline() -> Result<(), Box<dyn Error>> {
    let set_cmdline_path = build_test_binary("set_cmdline_from_stdin", "testbin")?;
    let mut set_cmdline = std::process::Command::new(set_cmdline_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let mut child_stdin = set_cmdline.stdin.take().unwrap();
    let mut child_stdout = linereader::LineReader::new(set_cmdline.stdout.take().unwrap());
    let engine = GeneralPurpose::new(&STANDARD, Default::default());

    let pid = Pid::from_u32(set_cmdline.id());
    println!("{pid}");
    let mut system = sysinfo::System::new();

    for case_str in ["TesT CMDLInE set\0-n lol"] {
        let case_base64 = engine
            .encode(case_str)
            .bytes()
            .filter(|ch| ch != &b'\n')
            .collect::<Vec<_>>();
        child_stdin.write(&case_base64)?;
        child_stdin.write(&[b'\n'])?;
        child_stdin.flush()?;

        if let Some(line) = child_stdout.next_line() {
            line?;
        }

        let args = case_str
            .split('\0')
            .map(|s| s.to_owned())
            .collect::<Vec<String>>();
        assert!(system.refresh_process_specifics(
            pid,
            ProcessRefreshKind::new().with_cmd(sysinfo::UpdateKind::Always)
        ));
        let set_cmdline_process = system.process(pid).expect("set-cmdline exits unexpectedly");
        assert_eq!(set_cmdline_process.cmd(), &args);
    }

    Ok(())
}
