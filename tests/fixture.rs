//! Ponta a ponta no app de exemplo: scan → apply --all → idempotência → undo.

use std::path::{Path, PathBuf};
use std::process::Command;

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for e in std::fs::read_dir(from).unwrap().flatten() {
        let name = e.file_name();
        if name == "node_modules" || name == ".next" {
            continue;
        }
        let (src, dst) = (e.path(), to.join(&name));
        if src.is_dir() {
            copy_dir(&src, &dst);
        } else {
            std::fs::copy(&src, &dst).unwrap();
        }
    }
}

fn snapshot(dir: &Path) -> Vec<(PathBuf, String)> {
    let mut out = vec![];
    fn walk(base: &Path, d: &Path, out: &mut Vec<(PathBuf, String)>) {
        for e in std::fs::read_dir(d).unwrap().flatten() {
            let p = e.path();
            if p.file_name().unwrap() == ".alltheani" {
                continue;
            }
            if p.is_dir() {
                walk(base, &p, out);
            } else {
                out.push((
                    p.strip_prefix(base).unwrap().to_path_buf(),
                    std::fs::read_to_string(&p).unwrap_or_default(),
                ));
            }
        }
    }
    walk(dir, dir, &mut out);
    out.sort();
    out
}

fn run(dir: &Path, args: &[&str]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_alltheani"))
        .args(args)
        .arg(dir)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{args:?} falhou: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).into_owned()
}

fn scan(dir: &Path) -> Vec<serde_json::Value> {
    serde_json::from_str(&run(dir, &["scan", "--json"])).unwrap()
}

#[test]
fn fixture_end_to_end() {
    let tmp = tempfile::tempdir().unwrap();
    let dir = tmp.path().join("app");
    copy_dir(&Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/next-app"), &dir);
    let before = snapshot(&dir);

    let items = scan(&dir);
    let cats: Vec<&str> = items.iter().map(|i| i["category"].as_str().unwrap()).collect();
    for c in [
        "page",
        "theme",
        "nav",
        "tabs",
        "modal",
        "toast",
        "accordion",
        "progress",
        "list",
        "button",
        "link",
    ] {
        assert!(cats.contains(&c), "faltou {c}: {cats:?}");
    }
    assert!(
        items.iter().any(|i| i["state"] == "animated"),
        "o botão .fancy já é animado"
    );

    run(&dir, &["apply", "--all", "--yes"]);
    let after = scan(&dir);
    for i in &after {
        assert!(i["state"] == "ours" || i["state"] == "animated", "não suavizado: {i}");
    }
    let layout = std::fs::read_to_string(dir.join("app/layout.tsx")).unwrap();
    assert!(layout.contains("import \"../alltheani/alltheani.css\";"));
    assert!(layout.contains("<AniTransitions />"));
    let toggle = std::fs::read_to_string(dir.join("components/theme-toggle.tsx")).unwrap();
    assert!(toggle.contains("onClick={withViewTransition(toggleTheme)}"));
    let nav = std::fs::read_to_string(dir.join("components/nav.tsx")).unwrap();
    assert!(nav.contains("<SlidingIndicator />") && nav.contains("main-nav ata-indicator-host"));
    let modal = std::fs::read_to_string(dir.join("components/modal.tsx")).unwrap();
    assert!(
        modal.contains(r#"cn("btn", open && "btn-on", "ata-press ata-d-150")"#),
        "{modal}"
    );
    assert!(dir.join("alltheani.config.json").exists() && dir.join("docs/transicoes.md").exists());

    // Rodar de novo não muda nada.
    assert!(run(&dir, &["apply", "--all", "--yes"]).contains("Nada a mudar"));

    // Desfazer volta ao original.
    run(&dir, &["undo"]);
    assert_eq!(snapshot(&dir), before);
}
