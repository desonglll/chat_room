use std::{env, fmt::Write as _, fs, path::Path, path::PathBuf, process::Command};

fn main() {
    for path in [
        "web/index.html",
        "web/package.json",
        "web/bun.lock",
        "web/tsconfig.json",
        "web/vite.config.ts",
        "web/src",
        "web/public",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }

    if !Path::new("web/node_modules/.bin/vite").exists() {
        let install = Command::new("bun")
            .args(["install", "--frozen-lockfile"])
            .current_dir("web")
            .status()
            .expect("run Bun; install Bun before running Cargo");
        assert!(install.success(), "Bun dependency installation failed");
    }

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("web");
    let status = Command::new("bun")
        .args(["run", "build", "--", "--outDir"])
        .arg(&output_dir)
        .arg("--emptyOutDir")
        .current_dir("web")
        .status()
        .expect("run Bun; install Bun and run `cd web && bun install`");

    assert!(status.success(), "Vue web build failed");
    generate_asset_manifest(&output_dir);
}

fn generate_asset_manifest(output_dir: &Path) {
    let asset_dir = output_dir.join("assets");
    let mut names: Vec<String> = fs::read_dir(&asset_dir)
        .expect("read built web assets")
        .map(|entry| entry.expect("read web asset entry").file_name())
        .map(|name| name.to_string_lossy().into_owned())
        .collect();
    names.sort();

    let mut source = String::from("const GENERATED_ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for name in names {
        let content_type = match Path::new(&name)
            .extension()
            .and_then(|value| value.to_str())
        {
            Some("css") => "text/css; charset=utf-8",
            Some("js") => "text/javascript; charset=utf-8",
            _ => "application/octet-stream",
        };
        writeln!(
            source,
            "    ({name:?}, {content_type:?}, include_bytes!(concat!(env!(\"OUT_DIR\"), \"/web/assets/{name}\"))),"
        )
        .expect("write asset manifest entry");
    }
    source.push_str("];\n");
    fs::write(
        output_dir
            .parent()
            .expect("web build output has a parent")
            .join("web_assets.rs"),
        source,
    )
    .expect("write web asset manifest");
}
