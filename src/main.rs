//! alltheani — encontra e suaviza as transições de um projeto Next.js, num painel no terminal.

mod codemod;
mod fsops;
mod model;
mod report;
mod runtime;
mod scan;
mod tui;

use model::{Config, State};
use std::io::Write;
use std::path::{Path, PathBuf};

const HELP: &str = "alltheani — transições suaves para projetos Next.js

Uso:
  alltheani [pasta]                 abre o painel (padrão: pasta atual)
  alltheani scan [pasta] [--json]   lista os pontos de transição
  alltheani apply [pasta] [--all] [--yes]
                                aplica o alltheani.config.json (--all = suavizar tudo).
                                Sem --yes só mostra o diff.
  alltheani undo [pasta]            desfaz a última aplicação
  alltheani --version

Docs: https://github.com/caioribi7/alltheani";

fn main() {
    if let Err(e) = run() {
        eprintln!("\x1b[1;31m✗\x1b[0m {e:#}");
        std::process::exit(1);
    }
}

fn run() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags: Vec<&str> = args.iter().filter(|a| a.starts_with('-')).map(String::as_str).collect();
    let pos: Vec<&str> = args
        .iter()
        .filter(|a| !a.starts_with('-'))
        .map(String::as_str)
        .collect();
    if flags.contains(&"--help") || flags.contains(&"-h") {
        println!("{HELP}");
        return Ok(());
    }
    if flags.contains(&"--version") || flags.contains(&"-V") {
        println!("alltheani {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let (cmd, dir) = match pos.first() {
        Some(&c @ ("scan" | "apply" | "undo")) => (c, pos.get(1).copied()),
        other => ("tui", other.copied()),
    };
    let root = PathBuf::from(dir.unwrap_or("."));
    if !root.is_dir() {
        anyhow::bail!("Pasta não encontrada: {}", root.display());
    }
    match cmd {
        "scan" => cmd_scan(&root, flags.contains(&"--json")),
        "apply" => cmd_apply(
            &root,
            flags.contains(&"--all"),
            flags.contains(&"--yes") || flags.contains(&"-y"),
        ),
        "undo" => {
            let n = fsops::undo(&root)?;
            println!("↺ {n} arquivo(s) restaurado(s).");
            Ok(())
        }
        _ => tui::run(&root),
    }
}

fn cmd_scan(root: &Path, json: bool) -> anyhow::Result<()> {
    let cfg = Config::load(root)?;
    let p = scan::scan(root, &cfg.runtime_dir)?;
    if json {
        let list: Vec<_> = p
            .items
            .iter()
            .map(|i| {
                serde_json::json!({
                    "id": i.id, "file": i.file, "line": i.line, "category": i.category.key(),
                    "tag": i.tag, "state": format!("{:?}", i.state).to_lowercase(), "note": i.note,
                })
            })
            .collect();
        println!("{}", serde_json::to_string_pretty(&list)?);
        return Ok(());
    }
    for w in &p.warnings {
        eprintln!("! {w}");
    }
    for i in &p.items {
        println!(
            "{:<12} {:<14} {}:{}  <{}>",
            i.category.key(),
            i.state.label(),
            i.file,
            i.line,
            i.tag
        );
    }
    let todo = p
        .items
        .iter()
        .filter(|i| matches!(i.state, State::None | State::Instant))
        .count();
    println!(
        "\n{} pontos, {} sem transição. Rode `alltheani` para escolher e aplicar.",
        p.items.len(),
        todo
    );
    Ok(())
}

fn cmd_apply(root: &Path, all: bool, yes: bool) -> anyhow::Result<()> {
    let mut cfg = Config::load(root)?;
    if all {
        cfg.smooth_everything();
    }
    let p = scan::scan(root, &cfg.runtime_dir)?;
    for w in &p.warnings {
        eprintln!("! {w}");
    }
    let changes = codemod::plan(&p, &cfg)?;
    if changes.is_empty() {
        println!("Nada a mudar.");
        return Ok(());
    }
    let mut out = std::io::stdout().lock();
    for c in &changes {
        if c.is_source() || !yes {
            writeln!(out, "{}", codemod::diff(c))?;
        }
    }
    if !yes {
        writeln!(
            out,
            "\n{} arquivo(s) mudariam. Rode de novo com --yes para aplicar.",
            changes.len()
        )?;
        return Ok(());
    }
    let n = changes.len();
    let dir = codemod::write_all(&p, &cfg, changes)?;
    writeln!(
        out,
        "✓ {} arquivo(s) escritos. Backup em {} — `alltheani undo` desfaz.",
        n,
        scan::rel(&p.root, &dir)
    )?;
    Ok(())
}
