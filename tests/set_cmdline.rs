use std::{error::Error, process};

use killmyargv::KillMyArgv;
use sysinfo::{Pid, ProcessRefreshKind};

#[test]
fn test_set_cmdline() -> Result<(), Box<dyn Error>> {
    // KillMyArgv is unsafe to call in parallel.
    // When we add new tests, we must either set this variable,
    // or use a `Mutex` lock
    //
    // assert_eq!(env!("RUST_TEST_THREADS"), 1);
    let pid = Pid::from_u32(process::id());
    let mut system = sysinfo::System::new();
    let kill_my_argv = KillMyArgv::new()?;

    for case_str in [
        "MAGIC USED HERE\0-p LOL",
        "Or\0This\0--help\0--secret=***********",
    ] {
        kill_my_argv.set(case_str.as_bytes());

        let args = case_str
            .split_terminator('\0')
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
