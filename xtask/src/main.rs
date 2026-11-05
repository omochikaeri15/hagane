mod android;
mod deploy;
mod ship;

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio, exit};

fn fail(message: &str) -> ! {
    eprintln!("Error: {message}");
    exit(1);
}

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().expect("xtask sits inside the workspace").to_path_buf()
}

fn target_dir(root: &Path) -> PathBuf {
    let output = Command::new("cargo")
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(root)
        .stderr(Stdio::inherit())
        .output()
        .expect("Failed to execute cargo metadata");
    let metadata = String::from_utf8_lossy(&output.stdout);
    let (_, rest) = metadata.split_once("\"target_directory\":\"").expect("cargo metadata named no target directory");
    let (target, _) = rest.split_once('"').expect("cargo metadata cut the target directory short");

    PathBuf::from(target)
}

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();

    match args.as_slice() {
        [] => ship::release(false),
        ["all"] => ship::release(true),
        ["recover"] => ship::recover(),
        ["debug", "android" | "apk"] => android::build(),
        ["deploy", "website" | "web"] => deploy::website(),
        ["deploy", "stop"] => deploy::stop(),
        [deploy::SERVE_COMMAND, root] => deploy::serve(Path::new(root)),
        unknown => {
            eprintln!("Unknown xtask arguments '{}'. Use 'cargo ship', 'cargo ship all', 'cargo ship recover', 'cargo debug android', 'cargo deploy website' or 'cargo deploy stop'.", unknown.join(" "));
            exit(2);
        }
    }
}
