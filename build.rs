use std::{env, fmt::Write as _, fs, path::Path, path::PathBuf, process::Command};

fn main() {
    let react_enabled = env::var_os("CARGO_FEATURE_REACT").is_some();
    let vue_enabled = env::var_os("CARGO_FEATURE_VUE").is_some();
    let api_only = env::var_os("CARGO_FEATURE_API_ONLY").is_some();
    assert!(
        usize::from(react_enabled) + usize::from(vue_enabled) + usize::from(api_only) <= 1,
        "choose one build target: --features react, --features vue, or --features api-only"
    );

    let output_dir = PathBuf::from(env::var_os("OUT_DIR").expect("OUT_DIR is set")).join("web");
    if api_only {
        generate_api_only_assets(&output_dir);
        return;
    }

    let (web_dir, client_name) = if react_enabled {
        ("web2", "React")
    } else {
        ("web", "Vue")
    };

    for path in [
        "index.html",
        "package.json",
        "bun.lock",
        "tsconfig.json",
        "vite.config.ts",
        "src",
        "public",
    ] {
        println!("cargo:rerun-if-changed={web_dir}/{path}");
    }

    let vite = Path::new(web_dir).join("node_modules/.bin/vite");
    if !vite.exists() {
        let install = Command::new("bun")
            .args(["install", "--frozen-lockfile"])
            .current_dir(web_dir)
            .status()
            .expect("run Bun; install Bun before running Cargo");
        assert!(install.success(), "Bun dependency installation failed");
    }

    let status = Command::new("bun")
        .args(["run", "build", "--", "--outDir"])
        .arg(&output_dir)
        .arg("--emptyOutDir")
        .current_dir(web_dir)
        .status()
        .unwrap_or_else(|_| panic!("run Bun; install Bun and run `cd {web_dir} && bun install`"));

    assert!(status.success(), "{client_name} web build failed");
    generate_asset_manifest(&output_dir);
}

fn generate_api_only_assets(output_dir: &Path) {
    fs::create_dir_all(output_dir).expect("create API-only asset directory");
    fs::write(
        output_dir.join("index.html"),
        "<!doctype html><title>API only</title>",
    )
    .expect("write API-only placeholder");
    fs::write(
        output_dir
            .parent()
            .expect("web build output has a parent")
            .join("web_assets.rs"),
        "const GENERATED_ASSETS: &[(&str, &str, &[u8])] = &[];\n",
    )
    .expect("write API-only asset manifest");
}

fn generate_asset_manifest(output_dir: &Path) {
    let mut files = Vec::new();
    collect_files(output_dir, output_dir, &mut files);
    files.retain(|(name, _)| name != "index.html");
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut source = String::from("const GENERATED_ASSETS: &[(&str, &str, &[u8])] = &[\n");
    for (name, path) in files {
        let content_type = content_type(&path);
        writeln!(
            source,
            "    ({name:?}, {content_type:?}, include_bytes!({path:?})),",
            path = path.to_string_lossy(),
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

fn collect_files(root: &Path, directory: &Path, files: &mut Vec<(String, PathBuf)>) {
    for entry in fs::read_dir(directory).expect("read built web assets") {
        let path = entry.expect("read web asset entry").path();
        if path.is_dir() {
            collect_files(root, &path, files);
        } else {
            let name = path
                .strip_prefix(root)
                .expect("asset is below output directory")
                .to_string_lossy()
                .replace('\\', "/");
            files.push((name, path));
        }
    }
}

fn content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|value| value.to_str()) {
        Some("css") => "text/css; charset=utf-8",
        Some("js") => "text/javascript; charset=utf-8",
        Some("json") => "application/json",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("ico") => "image/x-icon",
        Some("woff") => "font/woff",
        Some("woff2") => "font/woff2",
        Some("mp4") => "video/mp4",
        Some("webm") => "video/webm",
        Some("pdf") => "application/pdf",
        _ => "application/octet-stream",
    }
}
