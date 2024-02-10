use utils::{get_set_cmdline_path, set_cmdline, set_cmdline_with_child};

#[test]
fn test_set_cmdline_once() -> Result<()> {
    set_cmdline(["Hello?"], [vec!["Hello?"]])?;
    set_cmdline(["Hi\0there!"], [vec!["Hi", "there!"]])?;
    Ok(())
}

#[test]
fn test_set_cmdline_multiple_times() -> Result<()> {
    // Note: On a sort of OS, when `**argv` is not continuous to `envp`,
    // cmdline terminates on it's first NUL byte.
    set_cmdline(
        ["Hello?", "Hi\0there!"],
        [vec!["Hello?"], vec!["Hi", "there!"]],
    )?;
    Ok(())
}

#[test]
fn test_set_cmdline_truncate_max_len() -> Result<()> {
    let set_cmdline_path = get_set_cmdline_path()?;
    let mut child = Command::new(set_cmdline_path)
        .arg("true")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()?;
    let child_pid = child.id();
    let child_stdin = child.stdin.take().unwrap();
    let child_stdout = child.stdout.take().unwrap();

    let mut reader = linereader::LineReader::new(child_stdout);
    let max_len = reader.next_line().unwrap()?;
    let max_len = String::from_utf8_lossy(max_len).trim().parse()?;
    let expected = "o".repeat(max_len);
    let input = "o".repeat(max_len * 1);
    set_cmdline_with_child(
        [input],
        [vec![expected]],
        child_stdin,
        reader.into_inner(),
        child_pid,
    )?;
    Ok(())
}

mod utils;

use std::error::Error;
use std::process::{Command, Stdio};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
