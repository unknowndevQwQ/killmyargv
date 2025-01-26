use killmyargv::{argv_addrs, KillMyArgv};

use std::env::{args, args_os, set_var, vars_os};
use std::sync::Arc;
use std::{thread, time};

use log::{debug, info, set_max_level, trace, LevelFilter};
use spdlog::{default_logger, init_log_crate_proxy, log_crate_proxy, LogCrateProxy, Logger};

fn main() {
    init_log_crate_proxy().expect("users should only call `init_log_crate_proxy` function once");
    println!("Hi!");
    println!("argc: {}", args().len());

    set_max_level(LevelFilter::Trace);

    let custom_logger: Arc<Logger> = default_logger();

    // Logs will be output to `custom_logger`.
    let proxy: &'static LogCrateProxy = log_crate_proxy();
    custom_logger.set_level_filter(spdlog::LevelFilter::All);
    //proxy.set_logger(Some(custom_logger));

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
    thread::sleep(time::Duration::from_secs(3));

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
    thread::sleep(time::Duration::from_secs(3));

    mem.revert();
    println!("revert argv");
    printenv();
    thread::sleep(time::Duration::from_secs(3));

    mem.set(b"aaaaaaaaaaaaaaaaaaaa\0bbbbb12\088");
    println!("set le argv");
    printenv();
    thread::sleep(time::Duration::from_secs(3));
    println!("The end.");
}
