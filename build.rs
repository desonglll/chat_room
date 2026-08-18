use std::{env, path::Path, path::PathBuf, process::Command};

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
}
