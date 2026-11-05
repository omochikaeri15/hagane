use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::{fail, target_dir, workspace_root};

const DEFAULT_SDK: &str = "Android/Sdk";
const KEYSTORE: &str = "crates/mobile/release.keystore";
const KEYSTORE_ALIAS: &str = "bcc";
const KEYSTORE_PASSWORD: &str = "bcc-mobile";
const APK_FOLDER: &str = "release/apk";
const APK: &str = "Hagane-dev.apk";
const COMPILED: &str = "compiled";

fn android_home() -> PathBuf {
    let home = env::var_os("ANDROID_HOME").map(PathBuf::from).or_else(|| env::var_os("HOME").map(|home| Path::new(&home).join(DEFAULT_SDK)));

    home.filter(|path| path.is_dir()).unwrap_or_else(|| fail("no Android SDK was found: set ANDROID_HOME"))
}

fn version_key(path: &Path) -> Vec<u64> {
    path.file_name().map_or_else(Vec::new, |name| name.to_string_lossy().split('.').filter_map(|part| part.parse().ok()).collect())
}

fn android_ndk(sdk: &Path) -> PathBuf {
    let set = env::var_os("ANDROID_NDK_ROOT").or_else(|| env::var_os("ANDROID_NDK_HOME")).map(PathBuf::from);

    set.or_else(|| fs::read_dir(sdk.join("ndk")).ok()?.flatten().map(|entry| entry.path()).max_by_key(|path| version_key(path)))
        .unwrap_or_else(|| fail("no Android NDK was found: set ANDROID_NDK_ROOT or install one under the SDK's ndk folder"))
}

fn java_home() -> PathBuf {
    let found = env::var_os("JAVA_HOME").map(PathBuf::from).or_else(|| {
        let path = env::var_os("PATH")?;
        let javac = env::split_paths(&path).map(|folder| folder.join("javac")).find(|javac| javac.exists())?;

        Some(javac.canonicalize().ok()?.parent()?.parent()?.to_path_buf())
    });

    found.unwrap_or_else(|| fail("no JDK was found: set JAVA_HOME or put javac on the PATH"))
}

fn ensure_keystore(root: &Path, java: &Path) {
    let keystore = root.join(KEYSTORE);

    if keystore.exists() {
        return;
    }

    println!("> Creating the local test keystore...");

    let status = Command::new(java.join("bin/keytool"))
        .arg("-genkeypair")
        .arg("-keystore")
        .arg(&keystore)
        .args(["-alias", KEYSTORE_ALIAS, "-keyalg", "RSA", "-storepass", KEYSTORE_PASSWORD, "-keypass", KEYSTORE_PASSWORD, "-dname", "CN=Hagane"])
        .status()
        .expect("Failed to execute keytool");

    if !status.success() {
        fail("keytool could not create the local test keystore");
    }
}

pub(super) fn build() {
    let root = workspace_root();
    let sdk = android_home();
    let ndk = android_ndk(&sdk);
    let java = java_home();

    ensure_keystore(&root, &java);

    println!("> Building the dev APK with the NDK at {}...", ndk.display());

    let status = Command::new("cargo")
        .args(["apk", "build", "--release", "-p", "mobile", "--lib"])
        .current_dir(&root)
        .env("ANDROID_HOME", &sdk)
        .env("ANDROID_NDK_ROOT", &ndk)
        .env("ANDROID_NDK_HOME", &ndk)
        .env("JAVA_HOME", &java)
        .status()
        .expect("Failed to execute cargo apk; is cargo-apk installed?");

    if !status.success() {
        fail("the dev APK did not build");
    }

    let apk = target_dir(&root).join(APK_FOLDER).join(APK);

    if !apk.exists() {
        fail(&format!("the build finished without {}", apk.display()));
    }

    let compiled = root.join(COMPILED);

    fs::create_dir_all(&compiled).expect("Failed to create the compiled folder");
    fs::copy(&apk, compiled.join(APK)).expect("Failed to copy the APK into the compiled folder");

    println!("Built {}", compiled.join(APK).display());
}
