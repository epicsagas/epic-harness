use std::path::Path;
use std::process::Command;

fn main() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let app_dir = manifest.join("app");
    let dist_html = app_dir.join("dist").join("index.html");
    let asset = manifest.join("assets").join("dashboard.html");

    // Re-run if any app source file changes
    println!("cargo:rerun-if-changed=app/src");
    println!("cargo:rerun-if-changed=app/package.json");
    println!("cargo:rerun-if-changed=app/vite.config.ts");

    // Skip if no app/ directory (CI without Node)
    if !app_dir.exists() {
        return;
    }

    // Skip build if SKIP_DASHBOARD_BUILD is set (faster iteration)
    if std::env::var("SKIP_DASHBOARD_BUILD").is_ok() {
        return;
    }

    // Install deps if node_modules missing
    if !app_dir.join("node_modules").exists() {
        let status = Command::new("npm")
            .args(["install", "--prefer-offline"])
            .current_dir(&app_dir)
            .status();
        if status.map(|s| !s.success()).unwrap_or(true) {
            eprintln!("cargo:warning=npm install failed — dashboard will use existing assets/dashboard.html");
            return;
        }
    }

    // Build the Svelte app
    let status = Command::new("npm")
        .args(["run", "build"])
        .current_dir(&app_dir)
        .status();

    match status {
        Ok(s) if s.success() => {
            if dist_html.exists() {
                if let Err(e) = std::fs::copy(&dist_html, &asset) {
                    eprintln!("cargo:warning=Failed to copy dashboard.html: {e}");
                }
            } else {
                eprintln!("cargo:warning=npm build succeeded but dist/index.html not found");
            }
        }
        Ok(_) => eprintln!("cargo:warning=npm run build failed — dashboard will use existing assets/dashboard.html"),
        Err(e) => eprintln!("cargo:warning=Could not run npm: {e} — dashboard will use existing assets/dashboard.html"),
    }
}
