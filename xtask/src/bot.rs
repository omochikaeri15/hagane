use std::env;
use std::io::Write;
use std::process::{Command, Stdio};

use super::{fail, target_dir, workspace_root};

const HOST_VARIABLE: &str = "HAGANE_BOT_HOST";
const UPLOAD: &str = "/tmp/hagane-bot.new";
const INSTALLED: &str = "/usr/local/bin/hagane-bot";
const UNIT_PATH: &str = "/etc/systemd/system/hagane-bot.service";
const ENV_FILE: &str = "/etc/hagane/bot.env";
const SERVICE: &str = "hagane-bot";
const UNIT: &str = "[Unit]
Description=Hagane Discord bot
After=network-online.target
Wants=network-online.target

[Service]
User=hagane
Group=hagane
WorkingDirectory=/srv/hagane
EnvironmentFile=/etc/hagane/bot.env
ExecStart=/usr/local/bin/hagane-bot
Restart=always
RestartSec=10
NoNewPrivileges=yes
ProtectSystem=strict
ProtectHome=yes
PrivateTmp=yes
ReadWritePaths=/srv/hagane

[Install]
WantedBy=multi-user.target
";

fn glibc(text: &str) -> Vec<(u32, u32)> {
    text.split("GLIBC_")
        .skip(1)
        .filter_map(|rest| {
            let digits: String = rest.chars().take_while(|glyph| glyph.is_ascii_digit() || *glyph == '.').collect();
            let mut parts = digits.split('.').map(|part| part.parse::<u32>().ok());

            Some((parts.next()??, parts.next().flatten().unwrap_or(0)))
        })
        .collect()
}

fn stdout(command: &mut Command, what: &str) -> String {
    let output = command.output().unwrap_or_else(|err| fail(&format!("{what} could not be started: {err}")));

    if !output.status.success() {
        fail(&format!("{what} failed"));
    }

    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn run(command: &mut Command, what: &str) {
    let status = command.status().unwrap_or_else(|err| fail(&format!("{what} could not be started: {err}")));

    if !status.success() {
        fail(&format!("{what} failed"));
    }
}

pub(super) fn deploy(host: Option<&str>) {
    let host = host.map(str::to_owned).or_else(|| env::var(HOST_VARIABLE).ok()).filter(|host| !host.trim().is_empty()).unwrap_or_else(|| fail(&format!("name the server: 'cargo deploy bot user@host', or set {HOST_VARIABLE}")));
    let root = workspace_root();

    println!("> Building the bot...");
    run(Command::new("cargo").args(["build", "--release", "-p", "bot"]).current_dir(&root), "cargo build");

    let binary = target_dir(&root).join("release").join("bot");

    if !binary.exists() {
        fail(&format!("the build finished without {}", binary.display()));
    }

    let needed = glibc(&stdout(Command::new("objdump").arg("-T").arg(&binary), "objdump")).into_iter().max().unwrap_or((0, 0));
    let offered = glibc(&format!("GLIBC_{}", stdout(Command::new("ssh").args([&host, "getconf GNU_LIBC_VERSION"]), "the glibc check over ssh").trim().trim_start_matches("glibc "))).into_iter().max().unwrap_or((0, 0));

    if needed > offered {
        fail(&format!("the bot needs glibc {}.{} and {host} has {}.{}; build it on an older system or update the server", needed.0, needed.1, offered.0, offered.1));
    }

    println!("> Uploading to {host}...");
    run(Command::new("scp").args(["-q", &binary.display().to_string(), &format!("{host}:{UPLOAD}")]), "scp");

    println!("> Installing the service...");
    let mut unit = Command::new("ssh").args([&host, &format!("sudo tee {UNIT_PATH} > /dev/null")]).stdin(Stdio::piped()).spawn().unwrap_or_else(|err| fail(&format!("ssh could not be started: {err}")));

    unit.stdin.take().expect("ssh was spawned with a piped stdin").write_all(UNIT.as_bytes()).expect("the unit could not be sent");

    if !unit.wait().is_ok_and(|status| status.success()) {
        fail("the service unit could not be written");
    }

    let script = format!(
        "set -e; sudo install -o root -g root -m 755 {UPLOAD} {INSTALLED}; rm -f {UPLOAD}; sudo systemctl daemon-reload; sudo systemctl enable {SERVICE} > /dev/null 2>&1; \
         if sudo grep -q '^HAGANE_TOKEN=.' {ENV_FILE} 2>/dev/null; then sudo systemctl restart {SERVICE}; sleep 3; systemctl is-active {SERVICE}; \
         else echo 'No token in {ENV_FILE} yet, so the bot was installed but not started'; fi"
    );

    run(Command::new("ssh").args([&host, &script]), "the install over ssh");

    println!("Deployed. Follow its log with: ssh {host} 'journalctl -u {SERVICE} -f'");
}
