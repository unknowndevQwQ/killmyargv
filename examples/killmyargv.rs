use killmyargv::{argv_addrs, KillMyArgv};

use std::env::{args, args_os, set_var, vars_os};
use std::sync::Arc;

use log::{debug, info, set_max_level, trace, LevelFilter};
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

    println!("Hi!");
    println!("argc: {}", args().len());

    let mem = KillMyArgv::new().expect("msg");
    fn printenv() {
        if false {
            for i in args() {
                println!("std arg: {i:?}");
            }
            for (k, v) in vars_os() {
                println!("std env_os k&v: {k:?}={v:?}");
            }
        }
    }
    mem.set("eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee????\n".as_bytes());
    println!("set gr argv");
    printenv();
    pause!();

    match argv_addrs() {
        Ok(v) => {
            let (b, e) = v;
            println!("{b:?} {e:?}");
        }
        Err(e) => println!("addrs err: {e}"),
    }
    println!("cmdline max len: {}", mem.max_len());
    mem.set("char_vec!".as_bytes());
    set_var("key", "value");
    println!("set le argv and env");
    printenv();
    pause!();

    mem.revert();
    println!("revert argv");
    printenv();
    pause!();

    mem.set(b"aaaaaaaaaaaaaaaaaaaa\0bbbbb12\09988");
    println!("set le argv");
    printenv();
    pause!();

    mem.set("a".repeat(6144).as_bytes());
    println!("try set 6144 bytes to argv");
    printenv();
    pause!();

    mem.revert();
    println!("revert argv");
    printenv();
    pause!();

    if let Some(nonul_byte) = mem.nonul_byte() {
        let mut s = vec![b'a'; 6144];
        // If the length is greater than or equal to nonul_byte and null exists,
        // the set cmdline will be truncated at the null character,
        // but currently only nonul_byte is checked for null.
        s[nonul_byte - 1] = 0;
        mem.set(&s);
        println!("try set 6144(nonul_byte is null) bytes to argv");
        printenv();
        pause!();
    }
    println!("The end.");
}
