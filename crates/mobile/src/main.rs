use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(home) = mobile::paths::Home::local() else {
        eprintln!("No data folder could be found. Set HAGANE_HOME to a folder the app may use.");
        return ExitCode::FAILURE;
    };

    match mobile::run(home) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("The app stopped: {err}");
            ExitCode::FAILURE
        }
    }
}
