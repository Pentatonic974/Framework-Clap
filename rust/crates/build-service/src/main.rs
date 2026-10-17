//! build-service — pipeline de compilation d'un plugin depuis le JSON du graphe.
//!
//! Usage :
//! ```text
//! build-service <projet.json> [--check] [--armfuls clap,vst3,au] [--no-install] [--project-dir rust/]
//! ```
//!
//! Étapes :
//! 1. lecture + validation du `GraphSpec` ;
//! 2. génération du binaire via `cargo build --release -p clap-runtime`
//!    avec `CLAPS_GRAPH_JSON` pointant sur le graphe sérialisé ;
//! 3. renommage de l'artefact en `<meta.name>.clap` ;
//! 4. déploiement dans le répertoire système CLAP :
//!    Windows `%COMMONPROGRAMFILES%\CLAP`, macOS `~/Library/Audio/Plug-Ins/CLAP`,
//!    Linux `~/.clap` (débrayable avec `--no-install`).
//! 5. formats vst3/au : même binaire reconstruit avec les features
//!    `clap-runtime/vst3`,`auv2` (clap-wrapper-rs) — la mise en bundle du
//!    répertoire `.vst3`/`.component` dépend du DAW, la commande l'indique.
//!
//! La sortie standard est un JSON synthèse consommé par Flutter.

use graph_core::validation::validate;
use graph_core::spec::GraphSpec;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let args: Vec<String> = env::args().skip(1).collect();
    if args.is_empty() {
        eprintln!("usage : build-service <projet.json> [--check] [--formats clap,vst3,au] [--no-install] [--project-dir <rust>]");
        std::process::exit(2);
    }
    let mut project: Option<PathBuf> = None;
    let mut check = false;
    let mut no_install = false;
    let mut formats = String::from("clap");
    let mut project_dir: Option<PathBuf> = None;
    let mut it = args.iter();
    while let Some(a) = it.next() {
        match a.as_str() {
            "--check" => check = true,
            "--no-install" => no_install = true,
            "--formats" => formats = it.next().cloned().unwrap_or_else(|| "clap".into()),
            "--project-dir" => {
                project_dir = it.next().map(PathBuf::from);
            }
            other => project = Some(PathBuf::from(other)),
        }
    }
    let Some(project) = project else {
        eprintln!("fichier projet manquant");
        std::process::exit(2);
    };

    let result = run(&project, check, no_install, &formats, project_dir);
    match result {
        Ok(report) => println!("{}", serde_json::to_string_pretty(&report).unwrap_or_default()),
        Err(msg) => {
            eprintln!("BUILD ERROR: {msg}");
            std::process::exit(1);
        }
    }
}

#[derive(serde::Serialize)]
struct BuildReport {
    ok: bool,
    format_ok: Vec<String>,
    artifact_path: Option<String>,
    install_path: Option<String>,
    warnings: Vec<String>,
    errors: Vec<String>,
}

fn run(
    project: &Path,
    check: bool,
    no_install: bool,
    formats: &str,
    project_dir: Option<PathBuf>,
) -> Result<BuildReport, String> {
    let mut report = BuildReport {
        ok: false,
        format_ok: vec![],
        artifact_path: None,
        install_path: None,
        warnings: vec![],
        errors: vec![],
    };

    let json = fs::read_to_string(project).map_err(|e| format!("lecture impossible : {e}"))?;
    let spec = GraphSpec::from_json(&json).map_err(|e| format!("JSON invalide : {e}"))?;
    let validation = validate(&spec);
    if !validation.is_ok() {
        report.errors = validation.errors.clone();
        return Ok(report);
    }
    report.warnings.extend(validation.warnings.clone());

    if check {
        report.ok = true;
        return Ok(report);
    }

    // --- répertoire du workspace Rust ---
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map(|p| p.parent().map(|p2| p2.to_path_buf()).unwrap_or_else(|| p.to_path_buf()))
        .unwrap_or_else(|| PathBuf::from("."));
    let rust_dir = project_dir.unwrap_or(manifest_dir);
    if !rust_dir.join("Cargo.toml").exists() {
        return Err(format!("workspace introuvable : {}", rust_dir.display()));
    }

    // --- graphe pour le runtime ---
    let build_tmp = std::env::temp_dir().join("claps-built-graph.json");
    fs::write(&build_tmp, &json).map_err(|e| format!("écriture graphe : {e}"))?;

    // cargo (la commande std::process::Command est asynchrone par ressource OS
    // transparante) — aucun envoi réseau.
    let status = Command::new("cargo")
        .arg("build")
        .arg("--release")
        .arg("-p")
        .arg("clap-runtime")
        .env("CLAPS_GRAPH_JSON", &build_tmp)
        .current_dir(&rust_dir)
        .status()
        .map_err(|e| format!("cargo introuvable : {e}"))?;
    if !status.success() {
        return Err("cargo build --release a échoué".into());
    }

    // --- artefact ---
    let target = rust_dir.join("target").join("release");
    let lib_name = cfg!(windows)
        .then(|| "clap_runtime.dll")
        .or_else(|| cfg!(target_os = "macos").then(|| "libclap_runtime.dylib"))
        .unwrap_or("libclap_runtime.so");
    let artifact = target.join(lib_name);
    if !artifact.exists() {
        return Err(format!("artefact introuvable : {}", artifact.display()));
    }

    let out_name = format!("{}.clap", spec.meta.name.replace(' ', "_"));
    let out_path = target.parent().unwrap().join("claps").join(&out_name);
    fs::create_dir_all(out_path.parent().unwrap()).ok();
    fs::copy(&artifact, &out_path).map_err(|e| format!("copie .clap : {e}"))?;
    report.artifact_path = Some(out_path.display().to_string());
    report.format_ok.push("clap".into());

    // --- VST3 / AU (features clap-wrapper) ---
    for fmt in formats.split(',').map(str::trim) {
        match fmt {
            "vst3" if cfg!(windows) => {
                let status = Command::new("cargo")
                    .arg("build")
                    .arg("--release")
                    .arg("-p")
                    .arg("clap-runtime")
                    .arg("--features")
                    .arg("vst3")
                    .env("CLAPS_GRAPH_JSON", &build_tmp)
                    .current_dir(&rust_dir)
                    .status()
                    .map_err(|e| format!("cargo introuvable : {e}"))?;
                if status.success() {
                    report.format_ok.push("vst3".into());
                    report.warnings.push(
                        "VST3 : copier le binaire dans un bundle `<Nom>.vst3/Contents/x86_64-win/` pour le déploiement hôte.".into(),
                    );
                } else {
                    report.errors.push("déclinaison vst3 a échoué (feature clap-wrapper)".into());
                }
            }
            "au" => {
                if cfg!(target_os = "macos") {
                    let status = Command::new("cargo")
                        .arg("build")
                        .arg("--release")
                        .arg("-p")
                        .arg("clap-runtime")
                        .arg("--features")
                        .arg("auv2")
                        .env("CLAPS_GRAPH_JSON", &build_tmp)
                        .current_dir(&rust_dir)
                        .status()
                        .map_err(|e| format!("cargo introuvable : {e}"))?;
                    if status.success() {
                        report.format_ok.push("au".into());
                        report.warnings.push(
                            "AUv2 : bundle `.component` attendu (Xcode/inkjet selon clap-wrapper).".into(),
                        );
                    } else {
                        report.errors.push("déclinaison au a échoué (feature clap-wrapper)".into());
                    }
                } else {
                    report.warnings.push("AU : réservé macOS (auv2 vérifié par clap-wrapper-rs)".into());
                }
            }
            "clap" => {}
            other => report.warnings.push(format!("format inconnu : {other}")),
        }
    }

    // --- installation système ---
    if !no_install {
        let mut installed = false;
        for dir in clap_install_dirs() {
            if fs::create_dir_all(&dir).is_err() {
                continue;
            }
            let dst = dir.join(&out_name);
            if fs::copy(&out_path, &dst).is_ok() {
                report.install_path = Some(dst.display().to_string());
                installed = true;
                break;
            }
        }
        if !installed {
            report.warnings.push(
                "aucun répertoire CLAP accessible (droits ?) : copier manuellement le fichier .clap".into(),
            );
        }
    }

    report.ok = report.errors.is_empty();
    Ok(report)
}

/// Répertoires CLAP candidats (spec de distribution CLAP) — premier accessible,
/// sinon écrasé par le suivant :
/// Windows : `%COMMONPROGRAMFILES%\CLAP` puis `%LOCALAPPDATA%\Programs\Common\CLAP`
/// (par utilisateur) puis `~/.clap` ; macOS `~/Library/Audio/Plug-Ins/CLAP` ;
/// Linux `~/.clap`.
fn clap_install_dirs() -> Vec<PathBuf> {
    if cfg!(windows) {
        let mut dirs = Vec::new();
        if let Ok(common) = env::var("COMMONPROGRAMFILES") {
            dirs.push(PathBuf::from(common).join("CLAP"));
        }
        if let Ok(local) = env::var("LOCALAPPDATA") {
            dirs.push(PathBuf::from(local).join("Programs").join("Common").join("CLAP"));
        }
        if let Some(home) = user_home() {
            dirs.push(home.join(".clap"));
        }
        dirs
    } else if cfg!(target_os = "macos") {
        user_home()
            .map(|h| vec![h.join("Library/Audio/Plug-Ins/CLAP")])
            .unwrap_or_default()
    } else {
        user_home().map(|h| vec![h.join(".clap")]).unwrap_or_default()
    }
}

fn user_home() -> Option<PathBuf> {
    env::var("USERPROFILE").or_else(|_| env::var("HOME")).ok().map(PathBuf::from)
}
