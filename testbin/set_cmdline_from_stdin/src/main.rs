use std::{
    error::Error,
    io::{stdin, BufRead},
};

use base64::{
    alphabet,
    engine::{general_purpose::GeneralPurpose, GeneralPurposeConfig},
    Engine,
};
use killmyargv::KillMyArgv;

fn main() -> Result<(), Box<dyn Error>> {
    let stdin = stdin().lock();
    let alphabet = alphabet::STANDARD;
    let config = GeneralPurposeConfig::default();
    let engine = GeneralPurpose::new(&alphabet, config);

    let kill_my_argv = KillMyArgv::new()?;

    if let Some(output_argv_max_len) = std::env::args().nth(1) {
        if "true" == &output_argv_max_len {
            println!("{}", kill_my_argv.max_len())
        }
    }

    for next_cmd_line in stdin.lines() {
        let cmd_line = next_cmd_line?;
        let cmd_line = engine.decode(&cmd_line)?;
        kill_my_argv.set(&cmd_line);

        println!("set done");
    }

    Ok(())
}
