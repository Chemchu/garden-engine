use std::process::ExitCode;

use site_settings::{Args, Fontsource, run};

#[tokio::main]
async fn main() -> ExitCode {
    let args = match Args::parse(std::env::args().skip(1)) {
        Ok(args) => args,
        Err(problem) => {
            eprintln!("{problem}");
            eprintln!(
                "usage: site-settings --request <issue body file> --summary <output file> [--issue <number>] [--config site.toml] [--static-dir static]"
            );
            return ExitCode::FAILURE;
        }
    };

    match run(&args, &Fontsource::from_env()).await {
        Ok(outcome) => {
            println!(
                "Updated {} ({} font files added, {} unused font files or folders removed)",
                args.config.display(),
                outcome.added.len(),
                outcome.removed.len()
            );
            ExitCode::SUCCESS
        }
        Err(problems) => {
            for problem in problems {
                eprintln!("- {problem}");
            }
            ExitCode::FAILURE
        }
    }
}
