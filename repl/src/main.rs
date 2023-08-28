
use std::path::PathBuf;

use rustyline::error::ReadlineError;
use rustyline::{Cmd, CompletionType, Config, EditMode, Editor, KeyEvent};

use anyhow::Result;
use clap::Parser;
use directories::ProjectDirs;

use hyperon::common::shared::Shared;

mod metta_shim;
use metta_shim::*;

mod config_params;
use config_params::*;

mod interactive_helper;
use interactive_helper::*;

#[derive(Parser)]
#[command(version, about)]
struct CliArgs {
    /// .metta files to execute.  `metta` will run in interactive mode if no files are supplied
    files: Vec<PathBuf>,

    /// Additional include directory paths
    #[arg(short, long)]
    include_paths: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let cli_args = CliArgs::parse();

    //Config directory will be here: TODO: Document this in README.
    // Linux: ~/.config/metta/
    // Windows: ~\AppData\Roaming\OpenCog\metta\config\
    // Mac: ~/Library/Application Support/org.OpenCog.metta/
    let mut repl_params = match ProjectDirs::from("org", "OpenCog",  "metta") {
        Some(proj_dirs) => ReplParams::from_config_dir(proj_dirs.config_dir()),
        None => {
            eprint!("Failed to initialize config!");
            ReplParams::default()
        }
    };
    repl_params.push_include_paths(cli_args.include_paths);
    let repl_params = Shared::new(repl_params);

    let mut metta = MettaShim::new(repl_params.clone());

    //If we have .metta files to run, then run them
    if cli_args.files.len() > 0 {

        //Treat all files except the last as imports, and don't print the output
        let (last_path, other_paths) = cli_args.files.split_last().unwrap();

        for import_file in other_paths {
            metta.load_metta_module(import_file.clone());
        }

        //Only print the output from the last path
        let metta_code = std::fs::read_to_string(last_path)?;
        metta.exec(metta_code.as_str());
        metta.inside_env(|metta| {
            for result in metta.result.iter() {
                println!("{result:?}");
            }
        });
        Ok(())

    } else {

        //Otherwise enter interactive mode
        start_interactive_mode(repl_params, &mut metta).map_err(|err| err.into())
    }
}

// To debug rustyline:
// RUST_LOG=rustyline=debug cargo run --example example 2> debug.log
fn start_interactive_mode(repl_params: Shared<ReplParams>, metta: &mut MettaShim) -> rustyline::Result<()> {

    //Init RustyLine
    let config = Config::builder()
        .history_ignore_space(true)
        .completion_type(CompletionType::List)
        .edit_mode(EditMode::Emacs)
        .build();
    let helper = ReplHelper::new();
    let mut rl = Editor::with_config(config)?;
    rl.set_helper(Some(helper));
    rl.bind_sequence(KeyEvent::alt('n'), Cmd::HistorySearchForward);
    rl.bind_sequence(KeyEvent::alt('p'), Cmd::HistorySearchBackward);
    if let Some(history_path) = &repl_params.borrow().history_file {
        if rl.load_history(history_path).is_err() {
            println!("No previous history found.");
        }
    }

    //The Interpreter Loop
    loop {
        let p = format!("> ");
        rl.helper_mut().expect("No helper").colored_prompt = format!("\x1b[1;32m{p}\x1b[0m");
        let readline = rl.readline(&p);
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())?;

                metta.exec(line.as_str());
                metta.inside_env(|metta| {
                    for result in metta.result.iter() {
                        println!("{result:?}");
                    }
                });
            }
            Err(ReadlineError::Interrupted) |
            Err(ReadlineError::Eof) => {
                break;
            }
            Err(err) => {
                println!("Error: {err:?}");
                break;
            }
        }
    }

    if let Some(history_path) = &repl_params.borrow().history_file {
        rl.append_history(history_path)?
    }

    Ok(())
}
