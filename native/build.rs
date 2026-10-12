use std::{
    env, fs,
    path::{Path, PathBuf},
};

#[path = "src/configuration/content_files.rs"]
mod content_files;

fn collect(root: &Path, _dir: &Path, files: &mut Vec<(String, PathBuf)>) {
    println!("cargo:rerun-if-changed={}", root.display());
    for name in content_files::read(root)
        .expect("read bundled content")
        .into_keys()
    {
        let path = root
            .join(&name)
            .canonicalize()
            .expect("resolve content file");
        files.push((name, path));
    }
}

/// The command registration list is also the diagnostic command authority.
fn diagnostic_commands(manifest: &Path) {
    let path = manifest.join("src/application/startup.rs");
    println!("cargo:rerun-if-changed={}", path.display());
    let source = fs::read_to_string(path).expect("read registered commands");
    let block = source
        .split_once("tauri::generate_handler![")
        .expect("command registry")
        .1
        .split_once(']')
        .expect("command registry end")
        .0;
    let names: Vec<_> = block
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(|v| v.rsplit("::").next().unwrap())
        .collect();
    assert!(names.len() > 20);
    let mut output = String::from(
        "#[derive(Debug, Clone, serde::Deserialize, serde::Serialize, ts_rs::TS)]\n#[serde(rename_all = \"snake_case\")]\npub enum DiagnosticCommand {\n",
    );
    for name in &names {
        assert!(name.bytes().all(|b| b.is_ascii_lowercase() || b == b'_'));
        let variant: String = name
            .split('_')
            .map(|word| format!("{}{}", word[..1].to_ascii_uppercase(), &word[1..]))
            .collect();
        output.push_str(&format!("{variant},\n"));
    }
    output.push_str("}\npub const DIAGNOSTIC_COMMAND_NAMES: &[&str] = &[");
    for name in names {
        output.push_str(&format!("{name:?},"));
    }
    output.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("diagnostic_commands.rs"),
        output,
    )
    .expect("generate diagnostic commands");
}

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        // NDK r27 needs explicit ELF alignment for Android's 16 KB page sizes.
        // https://developer.android.com/guide/practices/page-sizes
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,max-page-size=16384");
        println!("cargo:rustc-link-arg-cdylib=-Wl,-z,common-page-size=16384");
    }
    let manifest = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    diagnostic_commands(&manifest);
    let root = manifest.join("../content");
    let mut files = Vec::new();
    collect(&root, &root, &mut files);
    let bibliography = manifest
        .join("../references.bib")
        .canonicalize()
        .expect("resolve bibliography");
    println!("cargo:rerun-if-changed={}", bibliography.display());
    files.push(("references.bib".into(), bibliography));
    files.sort_by(|a, b| a.0.cmp(&b.0));
    for pair in files.windows(2) {
        assert_ne!(pair[0].0, pair[1].0, "duplicate bundled configuration path");
    }
    let mut source = String::from("const SEEDS: &[(&str, &str)] = &[\n");
    for (name, path) in files {
        source.push_str(&format!(
            "({name:?}, include_str!({:?})),\n",
            path.to_str().unwrap()
        ));
    }
    source.push_str("];\n");
    fs::write(
        PathBuf::from(env::var_os("OUT_DIR").unwrap()).join("config_seeds.rs"),
        source,
    )
    .expect("write bundled configuration manifest");
    let mut attributes = tauri_build::Attributes::new();
    if env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc")
    {
        // Tauri's resource manifest is linked only into Cargo binary targets.
        // Library tests also link UI code (TaskDialogIndirect), which requires
        // Common Controls v6 before the process can even list its tests.
        // Embed the same dependency through the linker for every linked target;
        // leave icons/version resources with Tauri, without a duplicate manifest.
        attributes = attributes
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTUAC:NO");
        println!(
            "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
        );
    }
    tauri_build::try_build(attributes).expect("build Tauri resources");
}
