use std::process::ExitCode;

fn main() -> ExitCode {
    match drive_study_core::validate_repo(std::path::Path::new(".")) {
        Ok(()) => {
            println!("Content check passed.");
            ExitCode::SUCCESS
        }
        Err(errors) => {
            eprintln!("Content check failed ({})", errors.len());
            for error in errors {
                eprintln!("- {error}");
            }
            ExitCode::from(1)
        }
    }
}
