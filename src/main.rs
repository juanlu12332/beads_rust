use beads_rust::cli::commands;
use beads_rust::cli::{Cli, Commands};
use beads_rust::logging::init_logging;
use clap::Parser;

fn main() {
    let cli = Cli::parse();

    // Initialize logging
    if let Err(e) = init_logging(cli.verbose, cli.quiet, None) {
        eprintln!("Failed to initialize logging: {e}");
        // Don't exit, just continue without logging or with basic stderr
    }

    let result = match cli.command {
        Commands::Init { prefix, force, .. } => commands::init::execute(prefix, force),
        Commands::Create(args) => commands::create::execute(args),
        Commands::Delete(args) => commands::delete::execute(&args),
        Commands::List(args) => commands::list::execute(&args, cli.json),
        Commands::Search(args) => commands::search::execute(&args, cli.json),
        Commands::Count(args) => commands::count::execute(&args, cli.json),
        Commands::Doctor => commands::doctor::execute(cli.json),
        Commands::Version => commands::version::execute(cli.json),
        cmd => {
            println!("Command {cmd:?} not yet implemented");
            Ok(())
        }
    };

    if let Err(e) = result {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
