use killmyargv::KillMyArgv;
use utils::set_cmdline;

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
    let max_len = KillMyArgv::new()?.max_len();
    let expected = "o".repeat(max_len);
    let input = "o".repeat(max_len + 1);
    set_cmdline([input], [vec![expected]])?;
    Ok(())
}

mod utils;

use std::error::Error;
type Result<T> = std::result::Result<T, Box<dyn Error>>;
