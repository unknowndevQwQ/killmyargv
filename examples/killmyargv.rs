use killmyargv::{argv_addrs, KillMyArgv};

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
    set_max_level(LevelFilter::Trace);
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
        Err(e) => error!("reget addrs err: {e}"),
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

    mem.set("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee????\n".as_bytes());
    info!("set gr argv");
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
    info!("cmdline max len: {}", mem.max_len());
    mem.set("char_vec!".as_bytes());
    set_var("key", "value");
    info!("set le argv and env");
    printenv();
    pause!();

    mem.revert();
    info!("revert argv");
    printenv();
    pause!();

    mem.set(b"aaaaaaaaaaaaaaaaaaaa\0bbbbb12\09988");
    info!("set le argv");
    printenv();
    pause!();

    mem.set("a".repeat(6144).as_bytes());
    info!("try set 6144 bytes to argv");
    printenv();
    pause!();

    mem.revert();
    info!("revert argv");
    printenv();
    pause!();

    if let Some(nonul_byte) = mem.nonul_byte() {
        let mut s = vec![b'a'; 6144];
        // If the length is greater than or equal to nonul_byte and null exists,
        // the set cmdline will be truncated at the null character,
        // but currently only nonul_byte is checked for null.
        s[nonul_byte - 1] = 0;
        mem.set(&s);
        info!("try set 6144(nonul_byte is null) bytes to argv");
        printenv();
        pause!();
    }
    error!("The end.");
}
