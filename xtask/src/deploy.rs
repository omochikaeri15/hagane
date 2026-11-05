use std::collections::HashMap;
use std::env;
use std::fs::{self, File};
use std::io::{self, BufRead, BufReader, ErrorKind, Write};
use std::net::{TcpListener, TcpStream};
use std::os::unix::process::CommandExt;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use super::{fail, target_dir, workspace_root};

pub(super) const SERVE_COMMAND: &str = "serve-website";

const ADDRESS: &str = "127.0.0.1:8080";
const URL: &str = "http://localhost:8080";
const WEBSITE: &str = "crates/website";
const BUNDLE: &str = "dx/website/release/web/public";
const LIVE: &str = "website-deploy";
const PID_FILE: &str = "website-deploy.pid";
const LOG_FILE: &str = "website-deploy.log";
const HEADERS_FILE: &str = "_headers";
const INDEX: &str = "index.html";
const WASM_FLAGS: &str = "-C target-feature=+simd128";
const WAIT_STEP: Duration = Duration::from_millis(100);
const WAIT_STEPS: u32 = 50;

type Headers = HashMap<String, Vec<String>>;

fn ours(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/cmdline")).is_ok_and(|line| line.contains(SERVE_COMMAND))
}

fn signal(pid: u32, name: &str) {
    if let Err(err) = Command::new("kill").args([name, &pid.to_string()]).status() {
        eprintln!("Warning: kill could not be executed: {err}");
    }
}

fn stop_last(target: &Path) -> bool {
    let pid_file = target.join(PID_FILE);
    let last = fs::read_to_string(&pid_file).ok().and_then(|text| text.trim().parse::<u32>().ok()).filter(|pid| ours(*pid));
    let stopped = last.is_some();

    if let Some(pid) = last {
        println!("> Stopping the last deploy (pid {pid})...");
        signal(pid, "-TERM");

        if (0..WAIT_STEPS).all(|_| {
            thread::sleep(WAIT_STEP);
            ours(pid)
        }) {
            signal(pid, "-KILL");
        }
    }

    if pid_file.exists()
        && let Err(err) = fs::remove_file(&pid_file)
    {
        eprintln!("Warning: {} could not be removed: {err}", pid_file.display());
    }

    stopped
}

fn wait_until_up(child: &mut Child, log: &Path) {
    for _ in 0..WAIT_STEPS {
        thread::sleep(WAIT_STEP);

        if let Ok(Some(_)) = child.try_wait() {
            let reason = fs::read_to_string(log).unwrap_or_default();

            fail(&format!("the website server stopped right away: {}", reason.trim()));
        }

        if TcpStream::connect(ADDRESS).is_ok() {
            return;
        }
    }

    fail(&format!("the website server never answered on {ADDRESS}; its log is {}", log.display()));
}

pub(super) fn website() {
    let root = workspace_root();
    let target = target_dir(&root);
    let bundle = target.join(BUNDLE);
    let live = target.join(LIVE);

    if bundle.exists() {
        println!("> Clearing the previous website bundle...");
        fs::remove_dir_all(&bundle).expect("Failed to clear the previous website bundle");
    }

    println!("> Building the website for release...");

    let status = Command::new("dx")
        .args(["build", "--release"])
        .current_dir(root.join(WEBSITE))
        .env("RUSTFLAGS", WASM_FLAGS)
        .status()
        .expect("Failed to execute dx; is dioxus-cli installed?");

    if !status.success() {
        fail("the website did not build; a deploy that was already running was left as it is");
    }

    if !bundle.join(INDEX).exists() {
        fail(&format!("the build finished without {}", bundle.join(INDEX).display()));
    }

    stop_last(&target);

    if live.exists() {
        fs::remove_dir_all(&live).expect("Failed to clear the last deploy's files");
    }

    fs::rename(&bundle, &live).expect("Failed to move the new bundle into place");
    fs::copy(root.join(WEBSITE).join(HEADERS_FILE), live.join(HEADERS_FILE)).expect("Failed to copy the headers file");

    let log = target.join(LOG_FILE);
    let output = File::create(&log).expect("Failed to create the deploy log");
    let mut child = Command::new(env::current_exe().expect("Failed to find the xtask binary"))
        .arg(SERVE_COMMAND)
        .arg(&live)
        .stdin(Stdio::null())
        .stdout(output.try_clone().expect("Failed to share the deploy log"))
        .stderr(output)
        .process_group(0)
        .spawn()
        .expect("Failed to start the website server");

    fs::write(target.join(PID_FILE), child.id().to_string()).expect("Failed to record the deploy's pid");
    wait_until_up(&mut child, &log);

    println!("Deployed at {URL} (pid {}). 'cargo deploy stop' stops it.", child.id());
}

pub(super) fn stop() {
    if !stop_last(&target_dir(&workspace_root())) {
        println!("No website deploy is running.");
    }
}

fn read_headers(root: &Path) -> Headers {
    let mut headers = Headers::new();
    let mut path = None;

    for line in fs::read_to_string(root.join(HEADERS_FILE)).unwrap_or_default().lines() {
        if line.trim().is_empty() {
            continue;
        }

        if line.starts_with(char::is_whitespace) {
            if let Some(path) = &path {
                headers.entry(String::clone(path)).or_default().push(line.trim().to_owned());
            }
        } else {
            path = Some(line.trim().to_owned());
        }
    }

    headers
}

fn mime(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("wasm") => "application/wasm",
        Some("css") => "text/css; charset=utf-8",
        Some("json" | "map") => "application/json",
        Some("txt" | "md") => "text/plain; charset=utf-8",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("avif") => "image/avif",
        Some("svg") => "image/svg+xml",
        Some("ico") => "image/x-icon",
        Some("ttf") => "font/ttf",
        Some("otf") => "font/otf",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("ogg") => "audio/ogg",
        _ => "application/octet-stream",
    }
}

fn locate(root: &Path, request: &str) -> PathBuf {
    let relative = Path::new(request.trim_start_matches('/'));
    let file = root.join(relative);
    let safe = relative.components().all(|part| matches!(part, Component::Normal(_)));

    if safe && file.is_file() { file } else { root.join(INDEX) }
}

fn answer(stream: &TcpStream, root: &Path, headers: &Headers) -> io::Result<()> {
    let mut reader = BufReader::new(stream);
    let mut line = String::new();

    reader.read_line(&mut line)?;

    let mut ignored = String::new();

    while reader.read_line(&mut ignored)? > 0 && !matches!(ignored.as_str(), "\r\n" | "\n") {
        ignored.clear();
    }

    let mut parts = line.split_ascii_whitespace();
    let (method, target) = (parts.next().unwrap_or_default(), parts.next().unwrap_or("/"));
    let mut out = stream;

    if !matches!(method, "GET" | "HEAD") {
        return out.write_all(b"HTTP/1.1 405 Method Not Allowed\r\nAllow: GET, HEAD\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
    }

    let request = target.split(['?', '#']).next().unwrap_or("/");
    let path = locate(root, request);
    let mut file = File::open(&path)?;
    let length = file.metadata()?.len();
    let extra: String = headers.get(request).into_iter().flatten().map(|header| format!("{header}\r\n")).collect();

    write!(out, "HTTP/1.1 200 OK\r\nContent-Type: {}\r\nContent-Length: {length}\r\nCache-Control: no-cache\r\nConnection: close\r\n{extra}\r\n", mime(&path))?;

    if method == "GET" {
        io::copy(&mut file, &mut out)?;
    }

    Ok(())
}

fn hung_up(err: &io::Error) -> bool {
    matches!(err.kind(), ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted)
}

pub(super) fn serve(root: &Path) -> ! {
    let listener = TcpListener::bind(ADDRESS).unwrap_or_else(|err| fail(&format!("{ADDRESS} is taken, likely by another server such as dx serve: {err}")));
    let root = Arc::new(root.to_path_buf());
    let headers = Arc::new(read_headers(&root));

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let (root, headers) = (Arc::clone(&root), Arc::clone(&headers));

                thread::spawn(move || {
                    if let Err(err) = answer(&stream, &root, &headers)
                        && !hung_up(&err)
                    {
                        eprintln!("A request failed: {err}");
                    }
                });
            }
            Err(err) => eprintln!("A connection could not be accepted: {err}"),
        }
    }

    fail("the website server's listener closed");
}

#[cfg(test)]
mod tests {
    use std::process;

    use super::*;

    // Everything outside the bundle has to fall back to the page, never leak off the disk.
    #[test]
    fn a_request_never_climbs_out_of_the_bundle() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");

        assert_eq!(locate(&root, "/main.rs"), root.join("main.rs"));
        assert_eq!(locate(&root, "/../Cargo.toml"), root.join(INDEX));
        assert_eq!(locate(&root, "/cats/44"), root.join(INDEX));
        assert_eq!(locate(&root, "/"), root.join(INDEX));
    }

    #[test]
    fn a_headers_file_groups_its_lines_under_their_path() {
        let root = env::temp_dir().join(format!("xtask-headers-{}", process::id()));

        fs::create_dir_all(&root).unwrap();
        fs::write(root.join(HEADERS_FILE), "/assets/sw.js\n  Service-Worker-Allowed: /\n").unwrap();

        let headers = read_headers(&root);

        fs::remove_dir_all(&root).unwrap();
        assert_eq!(headers["/assets/sw.js"], ["Service-Worker-Allowed: /"]);
    }
}
