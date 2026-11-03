use std::env;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::process::Command;

const JAVA: &str = "android/Glue.java";
const CLASS_DIR: &str = "classes";
const DEX: &str = "classes.dex";
const MIN_API: &str = "26";

fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());

    if let Err(err) = slint_build::compile_with_config("ui/app.slint", config) {
        panic!("the Slint UI could not be compiled: {err}");
    }

    println!("cargo::rerun-if-changed={JAVA}");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android")
        && let Err(err) = dex()
    {
        panic!("the Android picker glue could not be compiled: {err}");
    }
}

fn dex() -> Result<(), String> {
    let out = PathBuf::from(env::var_os("OUT_DIR").ok_or("OUT_DIR is not set")?);
    let classes = out.join(CLASS_DIR);
    let sdk = env::var_os("ANDROID_HOME").or_else(|| env::var_os("ANDROID_SDK_ROOT")).map(PathBuf::from).ok_or("ANDROID_HOME is not set")?;
    let platform = newest(&sdk.join("platforms"))?.join("android.jar");
    let d8 = newest(&sdk.join("build-tools"))?.join("d8");
    let javac = env::var_os("JAVA_HOME").map(|home| PathBuf::from(home).join("bin").join("javac")).filter(|path| path.exists()).unwrap_or_else(|| PathBuf::from("javac"));

    match fs::remove_dir_all(&classes) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::NotFound => {}
        Err(err) => return Err(format!("{} could not be cleared: {err}", classes.display())),
    }

    fs::create_dir_all(&classes).map_err(|err| format!("{} could not be created: {err}", classes.display()))?;
    run(Command::new(&javac).args(["-source", "8", "-target", "8", "-Xlint:-options", "-encoding", "UTF-8", "-bootclasspath"]).arg(&platform).arg("-d").arg(&classes).arg(JAVA), "javac")?;

    let compiled = collect(&classes)?;

    run(Command::new(&d8).args(["--release", "--min-api", MIN_API, "--lib"]).arg(&platform).arg("--output").arg(&out).args(&compiled), "d8")?;

    if out.join(DEX).exists() { Ok(()) } else { Err(format!("d8 produced no {DEX}")) }
}

fn run(command: &mut Command, tool: &str) -> Result<(), String> {
    let output = command.output().map_err(|err| format!("{tool} could not be started: {err}"))?;

    if output.status.success() {
        Ok(())
    } else {
        Err(format!("{tool} failed: {}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr)))
    }
}

fn collect(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut found = Vec::new();

    for entry in fs::read_dir(dir).map_err(|err| format!("{} could not be read: {err}", dir.display()))? {
        let path = entry.map_err(|err| err.to_string())?.path();

        if path.is_dir() {
            found.extend(collect(&path)?);
        } else if path.extension().is_some_and(|ext| ext == "class") {
            found.push(path);
        }
    }

    Ok(found)
}

fn newest(dir: &Path) -> Result<PathBuf, String> {
    fs::read_dir(dir)
        .map_err(|err| format!("{} could not be read: {err}", dir.display()))?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .max_by_key(|path| version_key(path))
        .ok_or_else(|| format!("{} holds no installed versions", dir.display()))
}

fn version_key(path: &Path) -> Vec<u32> {
    path.file_name().and_then(|name| name.to_str()).map_or_else(Vec::new, |name| name.split(|c: char| !c.is_ascii_digit()).filter_map(|part| part.parse().ok()).collect())
}
