use std::{
    path::PathBuf,
    process::{Command, ExitCode},
};
fn run(args: &[&str]) -> Result<(), String> {
    run_tool("cargo", args)
}
fn run_tool(program: &str, args: &[&str]) -> Result<(), String> {
    if Command::new(program)
        .args(args)
        .status()
        .map_err(|e| e.to_string())?
        .success()
    {
        Ok(())
    } else {
        Err(format!("{program} {} failed", args.join(" ")))
    }
}
fn policy() -> Result<(), String> {
    let compiler = Command::new("rustc")
        .args(["--version", "--verbose"])
        .output()
        .map_err(|e| e.to_string())?;
    let compiler = String::from_utf8_lossy(&compiler.stdout);
    let host = compiler
        .lines()
        .find_map(|line| line.strip_prefix("host: "))
        .ok_or("Rust host target unavailable")?;
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--format-version=1",
            "--locked",
            "--offline",
            "--filter-platform",
            host,
        ])
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err("cargo metadata failed".into());
    }
    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).map_err(|e| e.to_string())?;
    for package in metadata["packages"]
        .as_array()
        .ok_or("Missing package metadata")?
    {
        if !package["source"].is_null() && package["license"].as_str().is_none() {
            return Err("Dependency license metadata missing; manual review required".into());
        }
    }
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("No dependency graph")?
    {
        let id = node["id"].as_str().unwrap_or("");
        let features = node["features"].as_array().ok_or("Missing features")?;
        for feature in features {
            let feature = feature.as_str().unwrap_or("");
            if ((id.contains("#eframe@") || id.contains("#egui@")) && feature == "persistence")
                || (id.contains("#eframe@") && feature == "glow")
                || (id.contains("#reqwest@") && feature == "cookies")
            {
                return Err(format!("Forbidden runtime feature: {id} / {feature}"));
            }
        }
    }
    let main = std::fs::read_to_string("apps/desktop/src/main.rs").map_err(|e| e.to_string())?;
    let compact = main.split_whitespace().collect::<String>();
    if !compact.contains("persist_window:false")
        || !compact.contains("fnpersist_egui_memory(&self)->bool{false}")
    {
        return Err("Native persistence controls changed".into());
    }
    let platform =
        std::fs::read_to_string("crates/platform/src/lib.rs").map_err(|e| e.to_string())?;
    if !platform.contains(".with_incognito(true)") {
        return Err("Authentication webview must be ephemeral".into());
    }
    println!(
        "Policy checks passed: no eframe persistence, one renderer, no REST cookie jar, ephemeral login requested."
    );
    Ok(())
}
fn copy_directory(source: &std::path::Path, destination: &std::path::Path) -> Result<(), String> {
    std::fs::create_dir_all(destination).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(source).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let target = destination.join(entry.file_name());
        if entry.file_type().map_err(|e| e.to_string())?.is_dir() {
            copy_directory(&entry.path(), &target)?;
        } else {
            std::fs::copy(entry.path(), target).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
fn package(voice: bool) -> Result<(), String> {
    let mut arguments = vec![
        "build",
        "--release",
        "--locked",
        "-p",
        "serein",
        "--no-default-features",
    ];
    if voice {
        arguments.extend(["--features", "voice"]);
    }
    run(&arguments)?;
    let root = PathBuf::from(if voice { "dist/voice" } else { "dist" });
    std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    let exe = if cfg!(windows) {
        "serein.exe"
    } else {
        "serein"
    };
    let destination = if cfg!(target_os = "macos") {
        let app = root.join("Serein.app/Contents");
        std::fs::create_dir_all(app.join("MacOS")).map_err(|e| e.to_string())?;
        std::fs::copy("packaging/macos/Info.plist", app.join("Info.plist"))
            .map_err(|e| e.to_string())?;
        app.join("MacOS").join(exe)
    } else {
        root.join(exe)
    };
    if cfg!(windows) {
        std::fs::copy(
            "packaging/windows/install-notifications.ps1",
            root.join("install-notifications.ps1"),
        )
        .map_err(|e| e.to_string())?;
    }
    let source = PathBuf::from("target/release").join(exe);
    if cfg!(target_os = "macos") {
        // macOS caches code signatures by inode. Replace the executable rather
        // than overwrite a previously launched, signed file in place.
        let staging = destination.with_extension("staging");
        match std::fs::remove_file(&staging) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
        std::fs::copy(&source, &staging).map_err(|e| e.to_string())?;
        std::fs::rename(&staging, &destination).map_err(|e| e.to_string())?;
    } else {
        std::fs::copy(&source, &destination).map_err(|e| e.to_string())?;
    }
    for file in [
        "README.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "THIRD_PARTY_NOTICES.md",
    ] {
        std::fs::copy(file, root.join(file)).map_err(|e| e.to_string())?;
    }
    let resources = if cfg!(target_os = "macos") {
        root.join("Serein.app/Contents/Resources")
    } else {
        root.clone()
    };
    std::fs::create_dir_all(resources.join("docs")).map_err(|e| e.to_string())?;
    std::fs::create_dir_all(resources.join("licenses")).map_err(|e| e.to_string())?;
    for file in ["NotoSansCJK-LICENSE.txt", "NotoSansArabic-OFL.txt"] {
        std::fs::copy(
            PathBuf::from("assets/fonts").join(file),
            resources.join("licenses").join(file),
        )
        .map_err(|e| e.to_string())?;
    }
    std::fs::copy(
        "assets/twemoji/LICENSE-GRAPHICS",
        resources.join("licenses/Twemoji-CC-BY-4.0.txt"),
    )
    .map_err(|e| e.to_string())?;
    std::fs::copy(
        "assets/twemoji/LICENSE-UNICODE",
        resources.join("licenses/Unicode-LICENSE.txt"),
    )
    .map_err(|e| e.to_string())?;
    copy_directory(
        std::path::Path::new("assets/licenses/files"),
        &resources.join("licenses/files"),
    )?;
    copy_directory(
        std::path::Path::new("assets/licenses/notifications"),
        &resources.join("licenses/notifications"),
    )?;
    if voice {
        copy_directory(
            std::path::Path::new("assets/licenses/voice"),
            &resources.join("licenses/voice"),
        )?;
        // Ship the corresponding modified MPL component source with every binary.
        copy_directory(
            std::path::Path::new("vendor/hpke-rs"),
            &resources.join("source/hpke-rs"),
        )?;
    }
    for file in [
        "README.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "THIRD_PARTY_NOTICES.md",
    ] {
        std::fs::copy(file, resources.join(file)).map_err(|e| e.to_string())?;
    }
    for entry in std::fs::read_dir("docs").map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        if entry.file_type().map_err(|e| e.to_string())?.is_file() {
            std::fs::copy(entry.path(), resources.join("docs").join(entry.file_name()))
                .map_err(|e| e.to_string())?;
        }
    }
    if cfg!(target_os = "macos") {
        // Seal only after every bundle resource has been staged. Ad-hoc signing
        // needs no identity and makes no Developer ID or notarization claim.
        let bundle = root.join("Serein.app");
        let bundle = bundle.to_str().ok_or("Invalid bundle path")?;
        run_tool("codesign", &["--force", "--sign", "-", bundle])?;
        run_tool("codesign", &["--verify", "--strict", bundle])?;
    }
    println!(
        "{} package executable: {} ({} bytes)",
        if cfg!(target_os = "macos") {
            "Locally ad-hoc signed (not notarized)"
        } else {
            "Unsigned"
        },
        destination.display(),
        std::fs::metadata(&destination)
            .map_err(|e| e.to_string())?
            .len()
    );
    Ok(())
}
fn main() -> ExitCode {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    std::env::set_current_dir(root).expect("workspace exists");
    let result = match std::env::args().nth(1).as_deref().unwrap_or("check") {
        "check" => run(&["fmt", "--all", "--", "--check"])
            .and_then(|_| {
                run(&[
                    "clippy",
                    "--workspace",
                    "--all-targets",
                    "--all-features",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ])
            })
            .and_then(|_| run(&["test", "--workspace", "--all-features", "--locked"]))
            .and_then(|_| run(&["check", "-p", "serein", "--no-default-features", "--locked"]))
            .and_then(|_| policy()),
        "policy" => policy(),
        "package" => package(false),
        "package-voice" => package(true),
        _ => Err("Use cargo xtask [check|policy|package|package-voice]".into()),
    };
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
