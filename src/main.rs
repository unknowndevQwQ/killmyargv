// lisence: expat/mit
mod utils;

use crate::utils::KillMyArgv;

use std::env::{args, args_os, set_var, vars_os};
use std::sync::Arc;
use std::{thread, time};

use log::{debug, info, set_max_level, trace, LevelFilter};
use spdlog::{default_logger, init_log_crate_proxy, log_crate_proxy, LogCrateProxy, Logger};

fn main() {
    println!("Hi!");
    println!("argc: {}", args().len());
    init_log_crate_proxy().expect("users should only call `init_log_crate_proxy` function once");

    set_max_level(LevelFilter::Trace);

    let custom_logger: Arc<Logger> = default_logger();

    // Logs will be output to `custom_logger`.
    let proxy: &'static LogCrateProxy = log_crate_proxy();
    proxy.set_logger(Some(custom_logger));

    let mem = KillMyArgv::new().expect("msg");
    fn printenv() {
        for i in args() {
            println!("std arg: {i:?}");
        }
        for (k, v) in vars_os() {
            println!("std env_os k&v: {k:?}={v:?}");
        }
    }
    mem.write(
        "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee????\n"
            .as_bytes()
            .to_vec(),
    );
    println!("set gr argv");
    printenv();
    thread::sleep(time::Duration::from_secs(3));

    unsafe {
        if let Ok(v) = KillMyArgv::argv_addrs() {
            let (b, e) = v;
            println!("{b:?} {e:?}");
        }
    }

    mem.write("char_vec!".as_bytes().to_vec());
    set_var("key", "value");
    println!("set le argv and env");
    printenv();
    thread::sleep(time::Duration::from_secs(3));

    mem.revert();
    println!("revert argv");
    printenv();
    thread::sleep(time::Duration::from_secs(3));

    mem.write(b"aaaaaaaaaaaaaaaaaaaa\0bbbbb12\088".to_vec());
    println!("set le argv");
    printenv();
    thread::sleep(time::Duration::from_secs(3));

    println!("The end.");
}
