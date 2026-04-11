use killmyargv::{argv_addrs, direct_argc_argv, KillMyArgv};

use std::env::{args, set_var, vars_os};
use std::process::{self, Command};
use std::slice::{from_raw_parts, from_raw_parts_mut};
use std::sync::Arc;

use clap::{Parser, Subcommand};
use log::{debug, error, info, set_max_level, warn, LevelFilter};
use pause_console::pause_console as pause;
use reedline::{
    default_emacs_keybindings, ColumnarMenu, DefaultCompleter, DefaultPrompt, EditCommand, Emacs,
    ExampleHighlighter, KeyCode, KeyModifiers, Keybindings, MenuBuilder, Reedline, ReedlineEvent,
    ReedlineMenu, Signal,
};
use spdlog::{
    default_logger,
    formatter::{pattern, PatternFormatter},
    init_log_crate_proxy, Logger,
};

#[derive(Parser)]
#[command(name = "")]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Commands>,

    #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
    default_args: Vec<String>,
}
#[derive(Subcommand)]
enum Commands {
    Read {
        #[arg(value_parser = parse_hex)]
        addr: usize,
        #[arg(allow_hyphen_values = true)]
        offset: isize,
        len: usize,
    },
    Write {
        #[arg(value_parser = parse_hex)]
        addr: usize,
        #[arg(allow_hyphen_values = true)]
        offset: isize,
        //#[arg(value_parser = parse_hex)]
        chars: Vec<u8>,
    },
}

const PS_CMD: &str = if cfg!(any(target_os = "illumos", target_os = "solaris")) {
    "ps -F -o user,pid,ppid,tty,s,time,comm,args -p"
} else {
    "ps -o user,pid,ppid,tty,stat,time,comm,args"
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

    let this_in_stack = 0u8;
    let this_by_alloc = Box::new(0u8);
    let this_in_data = "\0";
    info!(
        "some in stack: {:p}, some by alloc: {:p}, some in data: {:p}",
        &this_in_stack,
        this_by_alloc,
        this_in_data.as_ptr()
    );
    let mem = KillMyArgv::new().expect("try init fail");
    pause!();

    let (d_argc, d_argv) = unsafe { direct_argc_argv() };
    info!("direct argc: {}, argv: {:?}", d_argc, d_argv);

    info!("cmdline max len: {}", mem.max_len());
    let s = "a".repeat(mem.max_len()) + "\n";
    mem.set(s.as_bytes());
    info!("set max_len() + 1 args");
    printenv();
    printps();
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
    printps();
    pause!();

    mem.revert();
    info!("revert args");
    printenv();
    printps();
    pause!();

    let s = "a".repeat(20) + "\0" + "bbbbb12" + "\0" + "9988";
    mem.set(s.as_bytes());
    info!("set 32 chars args with two null char");
    printenv();
    printps();
    pause!();

    mem.set("a".repeat(6144).as_bytes());
    info!("set 6144 bytes args");
    printenv();
    printps();
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
        printps();
        pause!();
    }

    info!(
        "say again, some in stack: {:p}, some by alloc: {:p}, some in data: {:p}",
        &this_in_stack,
        this_by_alloc,
        this_in_data.as_ptr()
    );
    pause!();

    error!(
        "Now you can try to read/write memory by `read`/`write` command, {}, {}.\n{}, {}",
        "and exit by `exit` or `quit` command or press Ctrl-C/D",
        "if no command, default is use read command",
        "read/write cmd format: read <addr> <offset> <len>, write <addr> <offset> <chars_in_dec>",
        "I would prefer offset to be an optional parameter, but clap does not allow it."
    );
    let commands = vec![
        "bye".into(),
        "exit".into(),
        "read".into(),
        "write".into(),
        "quit".into(),
    ];
    let completer = Box::new(DefaultCompleter::new_with_wordlen(commands.clone(), 1));

    let completion_menu = Box::new(
        ColumnarMenu::default()
            .with_name("completion_menu")
            .with_columns(4)
            .with_column_width(None)
            .with_column_padding(2),
    );
    let mut keybindings = default_emacs_keybindings();
    add_menu_keybindings(&mut keybindings);

    let edit_mode = Box::new(Emacs::new(keybindings));

    let prompt = DefaultPrompt::default();
    let mut line_editor = Reedline::create()
        .with_highlighter(Box::new(ExampleHighlighter::new(commands.clone())))
        .with_completer(completer)
        .with_menu(ReedlineMenu::EngineCompleter(completion_menu))
        .with_edit_mode(edit_mode);
    loop {
        let sig = line_editor.read_line(&prompt);
        match sig {
            Ok(Signal::Success(line)) => {
                info!("input line: {line}");
                let raw_args = shlex::split(&line).unwrap_or_default();
                if raw_args.is_empty() {
                    continue;
                }
                if ["bye", "exit", "quit"].contains(&raw_args[0].as_str()) {
                    break;
                }

                let mut args = vec!["repl".to_string()];
                if !commands.contains(&raw_args[0]) && raw_args[0].starts_with("0x") {
                    args.push("read".to_string());
                }

                if let Some(mut split_args) = shlex::split(&line) {
                    args.append(&mut split_args);
                };
                match Cli::try_parse_from(args) {
                    Ok(cli) => match cli.cmd {
                        Some(Commands::Read { addr, offset, len }) => {
                            info!("read cmd: addr={addr:#x}, offset={offset:?}, len={len}");
                            if let Some(result) =
                                unsafe { read_addr(addr as *const u8, offset, len) }
                            {
                                info!("read result: {result:02x?}");
                            } else {
                                info!("read failed");
                            }
                        }
                        Some(Commands::Write {
                            addr,
                            offset,
                            chars,
                        }) => {
                            info!("write cmd: addr={addr:#x}, offset={offset:?}, chars={chars:?}");
                            if let Some(result) =
                                unsafe { write_addr(addr as *mut u8, offset, &chars) }
                            {
                                info!("write result: {result:?}");
                            } else {
                                info!("write failed");
                            }
                        }
                        None => {
                            info!("no subcommand, default args: {:?}", cli.default_args);
                        }
                    },
                    Err(err) => {
                        error!("parse error: {err}");
                    }
                }
            }
            Ok(Signal::CtrlC | Signal::CtrlD) => {
                info!("Ctrl-C/D received, exiting...");
                break;
            }
            Ok(s) => {
                info!("more signal: {s:?}");
            }
            Err(err) => {
                error!("Error reading line: {err}");
                break;
            }
        }
    }

    error!("The end.");
}

fn add_menu_keybindings(keybindings: &mut Keybindings) {
    keybindings.add_binding(
        KeyModifiers::NONE,
        KeyCode::Tab,
        ReedlineEvent::UntilFound(vec![
            ReedlineEvent::Menu("completion_menu".to_string()),
            ReedlineEvent::MenuNext,
        ]),
    );
    keybindings.add_binding(
        KeyModifiers::ALT,
        KeyCode::Enter,
        ReedlineEvent::Edit(vec![EditCommand::InsertNewline]),
    );
}

unsafe fn write_addr(addr: *mut u8, offset: isize, chars: &[u8]) -> Option<()> {
    if addr.is_null() {
        return None;
    }
    let ptr = addr.offset(offset);
    from_raw_parts_mut(ptr, chars.len()).copy_from_slice(chars);
    Some(())
}

unsafe fn read_addr(addr: *const u8, offset: isize, len: usize) -> Option<Vec<u8>> {
    if addr.is_null() {
        return None;
    }
    let ptr = addr.offset(offset);
    let mut result = Vec::new();
    from_raw_parts(ptr, len)
        .iter()
        .for_each(|&c| result.push(c));
    Some(result)
}

fn parse_hex(s: &str) -> Result<usize, String> {
    let s = s.trim();
    if s.starts_with("0x") || s.starts_with("0X") {
        usize::from_str_radix(&s[2..], 16).map_err(|e| format!("can't parse hex: {}", e))
    } else {
        s.parse::<usize>()
            .map_err(|e| format!("can't parse decimal: {}", e))
    }
}

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
fn printps() {
    let cmd = Command::new("sh")
        .args(&["-c", &format!("{} {}", PS_CMD, process::id())])
        .output()
        .expect("failed to execute ps command");
    warn!("ps output:\n{}", String::from_utf8_lossy(&cmd.stdout));
}
