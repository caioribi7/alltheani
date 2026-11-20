//! Monta o plano de mudanças (sem escrever nada): codemods mínimos + runtime + config + relatório.

use crate::model::{Category, ClassEdit, Config, Item, Preset, State, CONFIG_FILE};
use crate::scan::Project;
use crate::{report, runtime};
use oxc_allocator::Allocator;
use oxc_ast::ast::Statement;
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Change {
    pub path: String,
    pub old: Option<String>,
    pub new: String,
}

impl Change {
    pub fn is_source(&self) -> bool {
        matches!(
            Path::new(&self.path).extension().and_then(|e| e.to_str()),
            Some("tsx" | "jsx" | "js" | "ts")
        ) && !self.path.contains("/alltheani/")
    }
}

#[derive(Default)]
struct FileEdits {
    edits: Vec<(usize, usize, String)>,
    imports: Vec<(String, String)>, // (nome, módulo do runtime)
}

/// Caminho de import relativo de `from_file` até `target` (ambos relativos à raiz).
pub fn import_path(from_file: &str, target: &str) -> String {
    let from_dir: Vec<_> = Path::new(from_file)
        .parent()
        .unwrap_or(Path::new(""))
        .components()
        .collect();
    let to: Vec<_> = Path::new(target).components().collect();
    let common = from_dir.iter().zip(&to).take_while(|(a, b)| a == b).count();
    let mut p = PathBuf::new();
    for _ in common..from_dir.len() {
        p.push("..");
    }
    for c in &to[common..] {
        if let Component::Normal(s) = c {
            p.push(s);
        }
    }
    let s = p.to_string_lossy().replace('\\', "/");
    if s.starts_with("..") {
        s
    } else {
        format!("./{s}")
    }
}

/// Offset logo após o último import (ou diretivas como "use client").
fn import_anchor(file: &str, text: &str) -> usize {
    let alloc = Allocator::default();
    let st = SourceType::from_path(file).unwrap_or(SourceType::tsx());
    let st = if file.ends_with(".js") { st.with_jsx(true) } else { st };
    let ret = Parser::new(&alloc, text, st).parse();
    let last_import = ret
        .program
        .body
        .iter()
        .filter(|s| matches!(s, Statement::ImportDeclaration(_)))
        .map(|s| s.span().end as usize)
        .max();
    last_import
        .or_else(|| ret.program.directives.last().map(|d| d.span.end as usize))
        .unwrap_or(0)
}

fn class_insert(edit: &ClassEdit, classes: &str) -> Vec<(usize, usize, String)> {
    match edit {
        ClassEdit::Str { close, empty } => {
            vec![(
                *close,
                *close,
                if *empty {
                    classes.to_string()
                } else {
                    format!(" {classes}")
                },
            )]
        }
        ClassEdit::Template { close } => vec![(*close, *close, format!(" {classes}"))],
        ClassEdit::Call { close, sep } => vec![(*close, *close, format!("{sep}\"{classes}\""))],
        ClassEdit::Wrap { start, end } => {
            vec![
                (*start, *start, "[".into()),
                (*end, *end, format!(", \"{classes}\"].join(\" \")")),
            ]
        }
        ClassEdit::Missing { at } => vec![(*at, *at, format!(" className=\"{classes}\""))],
    }
}

fn plan_item(item: &Item, cfg: &Config, text: &str, fe: &mut FileEdits) {
    let Some(eff) = cfg.effective(item) else { return };
    let wanted = cfg.classes_for(item.category, &eff);

    // Classes: troca as `ata-` existentes ou acrescenta.
    if let Some((s, e)) = item.sv_literal {
        let old = &text[s..e];
        let mut toks: Vec<&str> = old.split_whitespace().filter(|t| !t.starts_with("ata-")).collect();
        toks.extend(wanted.iter().map(String::as_str));
        let lead = &old[..old.len() - old.trim_start().len()];
        let trail = &old[old.trim_end().len()..];
        let new = format!("{lead}{}{trail}", toks.join(" "));
        if new != old {
            fe.edits.push((s, e, new));
        }
    } else if !wanted.is_empty() {
        if let Some(edit) = &item.class_edit {
            fe.edits.extend(class_insert(edit, &wanted.join(" ")));
        }
    }

    if item.state == State::Ours {
        return; // helpers do runtime já colocados
    }
    match (eff.preset, item.category) {
        (Preset::Indicator, _) => {
            if let Some(at) = item.open_end {
                fe.edits
                    .push((at, at, format!("\n{}  <SlidingIndicator />", item.indent)));
                fe.imports.push(("SlidingIndicator".into(), "sliding-indicator".into()));
            }
        }
        (Preset::ViewTransition, Category::Page) => {
            if let Some(at) = item.open_end {
                fe.edits
                    .push((at, at, format!("\n{}  <AniTransitions />", item.indent)));
                fe.imports.push(("AniTransitions".into(), "transitions".into()));
            }
        }
        (Preset::ViewTransition, Category::Theme) => {
            if let Some((s, e)) = item.on_click {
                fe.edits.push((s, s, "withViewTransition(".into()));
                fe.edits.push((e, e, ")".into()));
                fe.imports.push(("withViewTransition".into(), "transitions".into()));
            }
        }
        _ => {}
    }
}

fn apply_edits(file: &str, text: &str, fe: &FileEdits, runtime_dir: &str, extra_top: Option<String>) -> String {
    let mut edits = fe.edits.clone();
    let mut import_lines: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (name, module) in &fe.imports {
        let spec = import_path(file, &format!("{runtime_dir}/{module}"));
        let names = import_lines.entry(spec).or_default();
        if !names.contains(name) {
            names.push(name.clone());
        }
    }
    let mut block = String::new();
    if let Some(line) = extra_top {
        block.push_str(&line);
    }
    for (spec, names) in import_lines {
        let names: Vec<_> = names
            .into_iter()
            .filter(|n| !text.contains(&format!("{n} }} from \"{spec}\"")))
            .collect();
        if !names.is_empty() {
            block.push_str(&format!("import {{ {} }} from \"{spec}\";\n", names.join(", ")));
        }
    }
    if !block.is_empty() {
        let at = import_anchor(file, text);
        let insert = if at == 0 {
            block
        } else {
            format!("\n{}", block.trim_end_matches('\n'))
        };
        edits.push((at, at, insert));
    }
    edits.sort_by(|a, b| b.0.cmp(&a.0).then(b.1.cmp(&a.1)));
    let mut out = text.to_string();
    for (s, e, t) in edits {
        out.replace_range(s..e, &t);
    }
    out
}

pub fn plan(project: &Project, cfg: &Config) -> anyhow::Result<Vec<Change>> {
    let root = &project.root;
    let rt = project.runtime_dir.clone();
    let mut per_file: BTreeMap<String, FileEdits> = BTreeMap::new();
    let mut texts: BTreeMap<String, String> = BTreeMap::new();

    for item in &project.items {
        if !texts.contains_key(&item.file) {
            texts.insert(item.file.clone(), std::fs::read_to_string(root.join(&item.file))?);
        }
        plan_item(
            item,
            cfg,
            &texts[&item.file],
            per_file.entry(item.file.clone()).or_default(),
        );
    }

    let mut changes = vec![];
    let push = |changes: &mut Vec<Change>, path: String, new: String| {
        let old = std::fs::read_to_string(root.join(&path)).ok();
        if old.as_deref() != Some(new.as_str()) {
            changes.push(Change { path, old, new });
        }
    };

    // Import do CSS no layout raiz.
    let css_spec = project
        .root_layout
        .as_ref()
        .map(|l| import_path(l, &format!("{rt}/alltheani.css")));
    if let (Some(layout), Some(spec)) = (&project.root_layout, &css_spec) {
        if !texts.contains_key(layout) {
            texts.insert(layout.clone(), std::fs::read_to_string(root.join(layout))?);
            per_file.entry(layout.clone()).or_default();
        }
        let _ = spec;
    }

    for (file, fe) in &per_file {
        let text = &texts[file];
        let extra = match (&project.root_layout, &css_spec) {
            (Some(l), Some(spec)) if l == file && !text.contains(spec.as_str()) => {
                Some(format!("import \"{spec}\";\n"))
            }
            _ => None,
        };
        let new = apply_edits(file, text, fe, &rt, extra);
        if &new != text {
            changes.push(Change {
                path: file.clone(),
                old: Some(text.clone()),
                new,
            });
        }
    }

    push(&mut changes, format!("{rt}/alltheani.css"), runtime::css(cfg));
    push(
        &mut changes,
        format!("{rt}/transitions.tsx"),
        runtime::transitions_tsx(cfg),
    );
    push(
        &mut changes,
        format!("{rt}/sliding-indicator.tsx"),
        runtime::sliding_indicator_tsx(),
    );
    push(&mut changes, CONFIG_FILE.into(), cfg.to_json());
    push(
        &mut changes,
        "docs/transicoes.md".into(),
        report::markdown(project, cfg),
    );
    if let Ok(gi) = std::fs::read_to_string(root.join(".gitignore")) {
        if !gi
            .lines()
            .any(|l| l.trim() == ".alltheani/" || l.trim() == ".alltheani")
        {
            let sep = if gi.ends_with('\n') || gi.is_empty() { "" } else { "\n" };
            changes.push(Change {
                path: ".gitignore".into(),
                old: Some(gi.clone()),
                new: format!("{gi}{sep}\n# alltheani (backups do desfazer)\n.alltheani/\n"),
            });
        }
    }
    Ok(changes)
}

/// Escreve as mudanças (com backup) e regenera o relatório com o estado novo.
pub fn write_all(project: &Project, cfg: &Config, mut changes: Vec<Change>) -> anyhow::Result<std::path::PathBuf> {
    let root = &project.root;
    let report_path = "docs/transicoes.md";
    if !changes.iter().any(|c| c.path == report_path) {
        let old = std::fs::read_to_string(root.join(report_path)).ok();
        changes.push(Change {
            path: report_path.into(),
            new: old.clone().unwrap_or_default(),
            old,
        });
    }
    let dir = crate::fsops::write(root, &changes)?;
    let fresh = crate::scan::scan(root, &cfg.runtime_dir)?;
    std::fs::write(root.join(report_path), report::markdown(&fresh, cfg))?;
    Ok(dir)
}

/// Diff unificado e legível de uma mudança.
pub fn diff(c: &Change) -> String {
    let old = c.old.as_deref().unwrap_or("");
    let d = similar::TextDiff::from_lines(old, &c.new);
    let a = if c.old.is_some() {
        format!("a/{}", c.path)
    } else {
        "/dev/null".into()
    };
    d.unified_diff()
        .context_radius(2)
        .header(&a, &format!("b/{}", c.path))
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_paths() {
        assert_eq!(
            import_path("src/app/layout.tsx", "src/alltheani/alltheani.css"),
            "../alltheani/alltheani.css"
        );
        assert_eq!(
            import_path("src/components/ui/x.tsx", "src/alltheani/transitions"),
            "../../alltheani/transitions"
        );
        assert_eq!(import_path("app/layout.tsx", "alltheani/x"), "../alltheani/x");
        assert_eq!(import_path("x.tsx", "alltheani/x"), "./alltheani/x");
    }

    #[test]
    fn class_inserts() {
        let src = r#"<button className={cn("a", b)}>x</button>"#;
        let close = src.find(")}").unwrap();
        let fe = FileEdits {
            edits: class_insert(&ClassEdit::Call { close, sep: ", " }, "ata-press"),
            imports: vec![],
        };
        assert_eq!(
            apply_edits("x.tsx", src, &fe, "alltheani", None),
            r#"<button className={cn("a", b, "ata-press")}>x</button>"#
        );

        let src = "<a className={styles.x}>x</a>";
        let (s, e) = (src.find("styles").unwrap(), src.find("}>").unwrap());
        let fe = FileEdits {
            edits: class_insert(&ClassEdit::Wrap { start: s, end: e }, "ata-press"),
            imports: vec![],
        };
        assert_eq!(
            apply_edits("x.tsx", src, &fe, "alltheani", None),
            r#"<a className={[styles.x, "ata-press"].join(" ")}>x</a>"#
        );
    }

    #[test]
    fn imports_after_use_client() {
        let src = "\"use client\";\n\nexport const A = () => <b />;\n";
        let fe = FileEdits {
            edits: vec![],
            imports: vec![("withViewTransition".into(), "transitions".into())],
        };
        let out = apply_edits("src/components/t.tsx", src, &fe, "src/alltheani", None);
        assert!(
            out.starts_with("\"use client\";\nimport { withViewTransition } from \"../alltheani/transitions\";\n"),
            "{out}"
        );
    }
}
