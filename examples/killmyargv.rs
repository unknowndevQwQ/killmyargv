use killmyargv::{argv_addrs, direct_argc_argv, KillMyArgv};

use std::env::{args, set_var, vars_os};
use std::sync::Arc;

use log::{debug, error, info, set_max_level, warn, LevelFilter};
use pause_console::pause_console as pause;
use spdlog::{
    default_logger,
    formatter::{pattern, PatternFormatter},
    init_log_crate_proxy, Logger,
};

fn main() {
    init_log_crate_proxy().expect("users should only call `init_log_crate_proxy` function once");
    set_max_level(LevelFilter::Debug);
    let logger: Arc<Logger> = default_logger();
    logger.set_level_filter(spdlog::LevelFilter::All);
    let formatter = Box::new(PatternFormatter::new(pattern!(
        "[{date} {time}.{nanosecond}] [{logger}] [{^{level}}] [{module_path}, {source}] [{pid}/{tid}] {payload}{eol}"
    )));
    for sink in logger.sinks() {
        sink.set_formatter(formatter.clone());
    }

    println!("Hi!, look at me!");
    warn!("I'm here!!!");
    info!("argc from std: {}", args().len());

    match argv_addrs() {
        Ok(v) => {
            let (b, e) = v;
            warn!("frist get argv start={b:?} end={e:?}");
        }
        Err(e) => error!("frist get addrs err: {e}"),
    }

    let mem = KillMyArgv::new().expect("try init fail");
    fn printenv() {
        if false {
            for i in args() {
                debug!("std arg: {i:?}");
            }
            for (k, v) in vars_os() {
                debug!("std env_os k&v: {k:?}={v:?}");
            }
        }
    }
    pause!();

    let (d_argc, d_argv) = unsafe { direct_argc_argv() };
    info!("direct argc: {}, argv: {:?}", d_argc, d_argv);

    info!("cmdline max len: {}", mem.max_len());
    let s = "a".repeat(mem.max_len()) + "\n";
    mem.set(s.as_bytes());
    info!("set max_len() + 1 args");
    printenv();
    pause!();

    warn!("Now you can get the correct address via argv_addrs() after initialization");
    match argv_addrs() {
        Ok(v) => {
            let (b, e) = v;
            warn!("reget argv start={b:?} end={e:?}");
        }
        Err(e) => error!("reget addrs err: {e}"),
    }
    mem.set(b"char_vec!");
    set_var("key", "value");
    info!("set 9 char args and env");
    printenv();
    pause!();

    mem.revert();
    info!("revert args");
    printenv();
    pause!();

    let s = "a".repeat(20) + "\0" + "bbbbb12" + "\0" + "9988";
    mem.set(s.as_bytes());
    info!("set 32 chars args with two null char");
    printenv();
    pause!();

    mem.set("a".repeat(6144).as_bytes());
    info!("set 6144 bytes args");
    printenv();
    pause!();

    if let Some(nonul_byte) = mem.nonul_byte() {
        let mut s = vec![b'a'; 6144];
        // If the length is greater than or equal to nonul_byte and null exists,
        // the set cmdline will be truncated at the null character,
        // but currently only nonul_byte is checked for null.
        s[nonul_byte - 1] = 0;
        mem.set(&s);
        info!("set 6144(nonul_byte is null) bytes args");
        printenv();
        pause!();
    }
    error!("The end.");
}
