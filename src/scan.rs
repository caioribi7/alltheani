//! Varre o projeto: TSX/JSX via oxc e CSS por texto, e lista os pontos de transição.

use crate::model::{Category, ClassEdit, Item, State};
use oxc_allocator::Allocator;
use oxc_ast::ast::*;
use oxc_ast_visit::{walk, Visit};
use oxc_parser::Parser;
use oxc_span::{GetSpan, SourceType};
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

pub struct Project {
    pub root: PathBuf,
    pub items: Vec<Item>,
    /// Layout raiz do App Router (relativo à raiz), se existir.
    pub root_layout: Option<String>,
    pub runtime_dir: String,
    pub warnings: Vec<String>,
}

const SKIP_DIRS: [&str; 8] = [
    "node_modules",
    ".next",
    ".git",
    "out",
    "dist",
    "build",
    ".alltheani",
    "coverage",
];

pub fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).to_string_lossy().replace('\\', "/")
}

pub fn default_runtime_dir(root: &Path) -> String {
    if root.join("src/app").is_dir() || root.join("src/pages").is_dir() {
        "src/alltheani".into()
    } else {
        "alltheani".into()
    }
}

pub fn find_root_layout(root: &Path) -> Option<String> {
    for dir in ["src/app", "app"] {
        for ext in ["tsx", "jsx", "js"] {
            let p = format!("{dir}/layout.{ext}");
            if root.join(&p).is_file() {
                return Some(p);
            }
        }
    }
    None
}

pub fn scan(root: &Path, runtime_dir_cfg: &str) -> anyhow::Result<Project> {
    let root = root.canonicalize()?;
    let runtime_dir = if runtime_dir_cfg.is_empty() {
        default_runtime_dir(&root)
    } else {
        runtime_dir_cfg.to_string()
    };
    let mut warnings = vec![];
    let is_next = std::fs::read_to_string(root.join("package.json"))
        .map(|s| s.contains("\"next\""))
        .unwrap_or(false);
    if !is_next {
        warnings.push(
            "Não achei `next` no package.json — o scan funciona, mas o runtime foi feito para o App Router.".into(),
        );
    }
    let root_layout = find_root_layout(&root);
    if root_layout.is_none() {
        warnings.push("Sem app/layout.tsx: importe o alltheani.css manualmente no seu layout ou _app.".into());
    }

    let mut code_files = vec![];
    let mut css_files = vec![];
    let walker = ignore::WalkBuilder::new(&root)
        .hidden(true)
        .filter_entry(|e| !SKIP_DIRS.contains(&e.file_name().to_string_lossy().as_ref()))
        .build();
    for entry in walker.flatten() {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let r = rel(&root, p);
        if r.starts_with(&format!("{runtime_dir}/")) || r.starts_with("fixtures/") {
            continue;
        }
        match p.extension().and_then(|e| e.to_str()) {
            Some("tsx" | "jsx") => code_files.push(p.to_path_buf()),
            Some("js") if !r.contains(".config.") => code_files.push(p.to_path_buf()),
            Some("css" | "scss") => css_files.push(p.to_path_buf()),
            _ => {}
        }
    }
    code_files.sort();

    let mut animated_classes = HashSet::new();
    let mut css_has_vt = false;
    for f in &css_files {
        let text = std::fs::read_to_string(f).unwrap_or_default();
        css_has_vt |= text.contains("view-transition");
        collect_animated_classes(&text, &mut animated_classes);
    }

    let mut items = vec![];
    for f in &code_files {
        let Ok(text) = std::fs::read_to_string(f) else { continue };
        let r = rel(&root, f);
        let is_root_layout = root_layout.as_deref() == Some(r.as_str());
        match scan_source(&r, &text, is_root_layout, &animated_classes, css_has_vt) {
            Ok(mut found) => items.append(&mut found),
            Err(e) => warnings.push(format!("{r}: {e}")),
        }
    }
    Ok(Project {
        root,
        items,
        root_layout,
        runtime_dir,
        warnings,
    })
}

/// Classes CSS cujas regras têm `transition` ou `animation`.
fn collect_animated_classes(css: &str, out: &mut HashSet<String>) {
    let mut rest = css;
    while let Some(open) = rest.find('{') {
        let selector = &rest[..open];
        let Some(close) = rest[open..].find('}') else { break };
        let body = &rest[open + 1..open + close];
        if body.contains("transition") || body.contains("animation") {
            for part in selector.split('.').skip(1) {
                let name: String = part
                    .chars()
                    .take_while(|c| c.is_alphanumeric() || *c == '-' || *c == '_')
                    .collect();
                if !name.is_empty() {
                    out.insert(name);
                }
            }
        }
        rest = &rest[open + close + 1..];
    }
}

pub fn scan_source(
    file: &str,
    text: &str,
    is_root_layout: bool,
    animated_classes: &HashSet<String>,
    css_has_vt: bool,
) -> anyhow::Result<Vec<Item>> {
    let alloc = Allocator::default();
    let st = SourceType::from_path(file).unwrap_or(SourceType::tsx());
    let st = if file.ends_with(".js") { st.with_jsx(true) } else { st };
    let ret = Parser::new(&alloc, text, st).parse();
    if ret.program.body.is_empty() && !ret.diagnostics.is_empty() {
        anyhow::bail!("não consegui ler o arquivo (erro de sintaxe)");
    }
    let mut v = Scanner {
        file,
        text,
        is_root_layout,
        animated_classes,
        css_has_vt,
        file_mentions_theme: file.to_lowercase().contains("theme"),
        list_roots: HashSet::new(),
        counters: BTreeMap::new(),
        items: vec![],
        line_starts: std::iter::once(0)
            .chain(text.match_indices('\n').map(|(i, _)| i + 1))
            .collect(),
    };
    v.visit_program(&ret.program);
    Ok(v.items)
}

struct Scanner<'s> {
    file: &'s str,
    text: &'s str,
    is_root_layout: bool,
    animated_classes: &'s HashSet<String>,
    css_has_vt: bool,
    file_mentions_theme: bool,
    list_roots: HashSet<u32>,
    counters: BTreeMap<Category, usize>,
    items: Vec<Item>,
    line_starts: Vec<usize>,
}

fn element_name(n: &JSXElementName) -> String {
    match n {
        JSXElementName::Identifier(i) => i.name.to_string(),
        JSXElementName::IdentifierReference(i) => i.name.to_string(),
        JSXElementName::NamespacedName(n) => format!("{}:{}", n.namespace.name, n.name.name),
        JSXElementName::MemberExpression(m) => {
            let mut s = String::new();
            member_name(m, &mut s);
            s
        }
        JSXElementName::ThisExpression(_) => "this".into(),
    }
}

fn member_name(m: &JSXMemberExpression, out: &mut String) {
    match &m.object {
        JSXMemberExpressionObject::IdentifierReference(i) => out.push_str(&i.name),
        JSXMemberExpressionObject::MemberExpression(inner) => member_name(inner, out),
        JSXMemberExpressionObject::ThisExpression(_) => out.push_str("this"),
    }
    out.push('.');
    out.push_str(&m.property.name);
}

fn attr<'b, 'a>(el: &'b JSXOpeningElement<'a>, name: &str) -> Option<&'b JSXAttribute<'a>> {
    el.attributes.iter().find_map(|a| match a {
        JSXAttributeItem::Attribute(a) => match &a.name {
            JSXAttributeName::Identifier(id) if id.name.as_str() == name => Some(&**a),
            _ => None,
        },
        _ => None,
    })
}

/// Valor estático de um atributo string (`role="tab"`).
fn attr_str<'b>(el: &JSXOpeningElement<'b>, name: &str) -> Option<String> {
    match attr(el, name)?.value.as_ref()? {
        JSXAttributeValue::StringLiteral(s) => Some(s.value.as_str().unwrap_or("").to_string()),
        JSXAttributeValue::ExpressionContainer(c) => match &c.expression {
            JSXExpression::StringLiteral(s) => Some(s.value.as_str().unwrap_or("").to_string()),
            _ => Some(String::new()),
        },
        _ => None,
    }
}

fn ends_with_any(name: &str, ends: &[&str]) -> bool {
    ends.iter().any(|e| name.ends_with(e))
}

fn returned_jsx_start(body: &ArrowFunctionBody) -> Option<u32> {
    fn from_expr(e: &Expression) -> Option<u32> {
        match e.without_parentheses() {
            Expression::JSXElement(el) => Some(el.span.start),
            _ => None,
        }
    }
    match body {
        ArrowFunctionBody::FunctionBody(b) => b.statements.iter().find_map(|s| match s {
            Statement::ReturnStatement(r) => r.argument.as_ref().and_then(from_expr),
            _ => None,
        }),
        other => other.as_expression().and_then(from_expr),
    }
}

const CLASS_HELPERS: [&str; 7] = ["cn", "clsx", "cx", "classNames", "classnames", "twMerge", "twJoin"];

impl<'s> Scanner<'s> {
    fn line_of(&self, offset: usize) -> usize {
        self.line_starts.partition_point(|&s| s <= offset)
    }

    fn indent_at(&self, offset: usize) -> String {
        let start = self.line_starts[self.line_of(offset) - 1];
        self.text[start..]
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect()
    }

    fn src(&self, start: u32, end: u32) -> &'s str {
        &self.text[start as usize..end as usize]
    }

    /// Analisa o className: como editar e se já tem classes `ata-`.
    fn class_info(&self, el: &JSXOpeningElement) -> (Option<ClassEdit>, Option<(usize, usize)>, String) {
        let Some(a) = attr(el, "className") else {
            let at = match &el.type_arguments {
                Some(t) => t.span.end,
                None => el.name.span().end,
            } as usize;
            return (Some(ClassEdit::Missing { at }), None, String::new());
        };
        let Some(value) = &a.value else {
            return (None, None, String::new());
        };
        let raw = self.src(a.span.start, a.span.end).to_string();
        let mut lits = vec![];
        collect_literals(value, &mut lits);
        let sv_literal = lits
            .into_iter()
            .find(|&(s, e)| self.text[s..e].split_whitespace().any(|t| t.starts_with("ata-")));
        let edit = match value {
            JSXAttributeValue::StringLiteral(s) => Some(ClassEdit::Str {
                close: s.span.end as usize - 1,
                empty: s.value.as_str().unwrap_or("").trim().is_empty(),
            }),
            JSXAttributeValue::ExpressionContainer(c) => match &c.expression {
                JSXExpression::StringLiteral(s) => Some(ClassEdit::Str {
                    close: s.span.end as usize - 1,
                    empty: s.value.as_str().unwrap_or("").trim().is_empty(),
                }),
                JSXExpression::TemplateLiteral(t) => Some(ClassEdit::Template {
                    close: t.span.end as usize - 1,
                }),
                JSXExpression::CallExpression(call) if matches!(&call.callee, Expression::Identifier(id) if CLASS_HELPERS.contains(&id.name.as_str())) =>
                {
                    let close = call.span.end as usize - 1;
                    let sep = match call.arguments.last() {
                        None => "",
                        Some(last) if self.text[last.span().end as usize..close].contains(',') => " ",
                        Some(_) => ", ",
                    };
                    Some(ClassEdit::Call { close, sep })
                }
                JSXExpression::EmptyExpression(_) => None,
                other => {
                    let sp = other.span();
                    Some(ClassEdit::Wrap {
                        start: sp.start as usize,
                        end: sp.end as usize,
                    })
                }
            },
            _ => None,
        };
        (edit, sv_literal, raw)
    }

    fn classify(&self, el: &JSXElement, name: &str, class_raw: &str, has_class: bool) -> Option<Category> {
        let o = &el.opening_element;
        let role = attr_str(o, "role").unwrap_or_default();
        let native = name.chars().next().is_some_and(|c| c.is_ascii_lowercase());
        let has_states = has_state_classes(class_raw);

        if self.is_root_layout && name == "body" {
            return Some(Category::Page);
        }
        if let Some(click) = attr(o, "onClick") {
            let click_src = self.src(click.span.start, click.span.end).to_lowercase();
            let themey = click_src.contains("theme") || click_src.contains("dark");
            if themey || (self.file_mentions_theme && (name == "button" || name == "Button")) {
                return Some(Category::Theme);
            }
        }
        // A partir daqui as categorias precisam de className. Componentes próprios sem
        // className podem não repassar a prop, então ficam de fora (o elemento nativo
        // dentro deles é encontrado no próprio arquivo do componente).
        if !native && !has_class && name != "Link" {
            return None;
        }
        if name == "nav" || role == "navigation" {
            return Some(Category::Nav);
        }
        if role == "tablist" || ends_with_any(name, &["TabsList", "TabList", "Segmented", "SegmentedControl"]) {
            return Some(Category::Tabs);
        }
        if name == "dialog"
            || role == "dialog"
            || role == "alertdialog"
            || attr(o, "aria-modal").is_some()
            || ends_with_any(
                name,
                &["Modal", "Dialog", "DialogContent", "SheetContent", "DrawerContent"],
            )
        {
            return Some(Category::Modal);
        }
        if role == "status"
            || role == "alert"
            || attr(o, "aria-live").is_some()
            || ends_with_any(name, &["Toast", "Snackbar"])
        {
            return Some(Category::Toast);
        }
        if role == "menu"
            || role == "listbox"
            || ends_with_any(name, &["MenuContent", "PopoverContent", "SelectContent", "Dropdown"])
        {
            return Some(Category::Dropdown);
        }
        if name == "details" || name.starts_with("Accordion") || name.starts_with("Collapsible") {
            return Some(Category::Accordion);
        }
        if name == "progress" || role == "progressbar" || ends_with_any(name, &["Progress", "ProgressBar"]) {
            return Some(Category::Progress);
        }
        if self.list_roots.contains(&el.span.start) {
            return Some(Category::List);
        }
        if name == "button" || name == "Button" || role == "button" {
            return Some(Category::Button);
        }
        if name == "a" || name == "Link" {
            return Some(Category::Link);
        }
        if has_states {
            return Some(Category::Interactive);
        }
        None
    }

    fn state(&self, el: &JSXElement, name: &str, cat: Category, class_raw: &str, sv: bool) -> State {
        let child_named = |n: &str| {
            el.children
                .iter()
                .any(|c| matches!(c, JSXChild::Element(e) if element_name(&e.opening_element.name) == n))
        };
        match cat {
            Category::Page => {
                if child_named("AniTransitions") {
                    State::Ours
                } else if self.css_has_vt || self.text.contains("startViewTransition") {
                    State::Animated
                } else {
                    State::Instant
                }
            }
            Category::Theme => {
                let click = attr(&el.opening_element, "onClick")
                    .map(|a| self.src(a.span.start, a.span.end))
                    .unwrap_or("");
                if click.contains("withViewTransition") {
                    State::Ours
                } else if click.contains("startViewTransition") {
                    State::Animated
                } else {
                    State::Instant
                }
            }
            Category::Nav | Category::Tabs if child_named("SlidingIndicator") => State::Ours,
            _ if sv => State::Ours,
            _ if name.starts_with("motion.") || is_animated_class(class_raw, self.animated_classes) => State::Animated,
            Category::Button | Category::Link | Category::Interactive => {
                if has_state_classes(class_raw) {
                    State::Instant
                } else {
                    State::None
                }
            }
            _ => State::Instant,
        }
    }

    fn note(&self, el: &JSXElement, cat: Category) -> Option<String> {
        match cat {
            Category::Nav | Category::Tabs => {
                let inner = self.src(el.span.start, el.span.end);
                let marks = ["aria-current", "aria-selected", "data-state", "data-active"];
                if marks.iter().any(|m| inner.contains(m)) {
                    None
                } else {
                    Some("o item ativo precisa de aria-current/aria-selected para o indicador achar".into())
                }
            }
            Category::Page => Some("cobre cliques em links; router.push() fica com o fallback".into()),
            _ => None,
        }
    }
}

fn has_state_classes(raw: &str) -> bool {
    [
        "hover:",
        "focus:",
        "focus-visible:",
        "active:",
        "aria-",
        "data-[",
        "group-hover:",
        "?",
        "&&",
    ]
    .iter()
    .any(|k| raw.contains(k))
}

fn is_animated_class(raw: &str, animated: &HashSet<String>) -> bool {
    if ["transition", "animate-", "duration-", "motion-safe:"]
        .iter()
        .any(|k| raw.contains(k))
    {
        return true;
    }
    raw.split(|c: char| !(c.is_alphanumeric() || c == '-' || c == '_'))
        .any(|t| animated.contains(t))
}

/// Spans (conteúdo sem aspas) de strings e trechos de template dentro de um valor.
fn collect_literals(value: &JSXAttributeValue, out: &mut Vec<(usize, usize)>) {
    struct Lits<'o>(&'o mut Vec<(usize, usize)>);
    impl<'a> Visit<'a> for Lits<'_> {
        fn visit_string_literal(&mut self, it: &StringLiteral<'a>) {
            self.0.push((it.span.start as usize + 1, it.span.end as usize - 1));
        }
        fn visit_template_element(&mut self, it: &TemplateElement<'a>) {
            self.0.push((it.span.start as usize, it.span.end as usize));
        }
    }
    Lits(out).visit_jsx_attribute_value(value);
}

impl<'a> Visit<'a> for Scanner<'_> {
    fn visit_call_expression(&mut self, it: &CallExpression<'a>) {
        if let Expression::StaticMemberExpression(m) = &it.callee {
            if m.property.name.as_str() == "map" {
                let start = match it.arguments.first().and_then(|a| a.as_expression()) {
                    Some(Expression::ArrowFunctionExpression(f)) => returned_jsx_start(&f.body),
                    _ => None,
                };
                if let Some(s) = start {
                    self.list_roots.insert(s);
                }
            }
        }
        walk::walk_call_expression(self, it);
    }

    fn visit_jsx_element(&mut self, el: &JSXElement<'a>) {
        let o = &el.opening_element;
        let name = element_name(&o.name);
        if name != "SlidingIndicator" && name != "AniTransitions" {
            let (class_edit, sv_literal, class_raw) = self.class_info(o);
            let has_class = attr(o, "className").is_some();
            if let Some(cat) = self.classify(el, &name, &class_raw, has_class) {
                let n = self.counters.entry(cat).or_default();
                let id = format!("{}:{}:{}", self.file, cat.key(), n);
                *n += 1;
                let state = self.state(el, &name, cat, &class_raw, sv_literal.is_some());
                let open_end = if el.closing_element.is_some() {
                    Some(o.span.end as usize)
                } else {
                    None
                };
                self.items.push(Item {
                    id,
                    file: self.file.to_string(),
                    line: self.line_of(o.span.start as usize),
                    category: cat,
                    state,
                    tag: name.clone(),
                    note: self.note(el, cat),
                    class_edit,
                    sv_literal,
                    open_end,
                    indent: self.indent_at(o.span.start as usize),
                    on_click: attr(o, "onClick").and_then(|a| match &a.value {
                        Some(JSXAttributeValue::ExpressionContainer(c)) => {
                            let sp = c.expression.span();
                            Some((sp.start as usize, sp.end as usize))
                        }
                        _ => None,
                    }),
                });
            }
        }
        walk::walk_jsx_element(self, el);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan_str(src: &str) -> Vec<Item> {
        scan_source("x.tsx", src, false, &HashSet::new(), false).unwrap()
    }

    #[test]
    fn detects_common_points() {
        let items = scan_str(
            r#"export default function P({ open, xs }: any) {
  return (
    <main>
      <nav><a href="/" aria-current="page">Início</a></nav>
      <button className="px-2 hover:bg-blue-500" onClick={() => {}}>Ok</button>
      <div role="dialog">Oi</div>
      <ul>{xs.map((x: any) => <li key={x}>{x}</li>)}</ul>
      <div role="progressbar"><div style={{ width: "50%" }} /></div>
      <button className="transition-colors">Já animado</button>
    </main>
  );
}"#,
        );
        let cats: Vec<_> = items.iter().map(|i| (i.category, i.state)).collect();
        assert!(cats.contains(&(Category::Nav, State::Instant)));
        assert!(cats.contains(&(Category::Link, State::None)));
        assert!(cats.contains(&(Category::Button, State::Instant)));
        assert!(cats.contains(&(Category::Modal, State::Instant)));
        assert!(cats.contains(&(Category::List, State::Instant)));
        assert!(cats.contains(&(Category::Progress, State::Instant)));
        assert!(cats.contains(&(Category::Button, State::Animated)));
        assert_eq!(items[0].line, 4);
    }

    #[test]
    fn class_edit_kinds() {
        let items = scan_str(
            r#"const a = <>
  <button className="a">1</button>
  <button className={cn("a", b,)}>2</button>
  <button className={`a ${b}`}>3</button>
  <button className={styles.x}>4</button>
  <button>5</button>
  <button className="a ata-press">6</button>
</>;"#,
        );
        let k: Vec<_> = items.iter().map(|i| i.class_edit.clone().unwrap()).collect();
        assert!(matches!(k[0], ClassEdit::Str { empty: false, .. }));
        assert!(matches!(k[1], ClassEdit::Call { sep: " ", .. }));
        assert!(matches!(k[2], ClassEdit::Template { .. }));
        assert!(matches!(k[3], ClassEdit::Wrap { .. }));
        assert!(matches!(k[4], ClassEdit::Missing { .. }));
        assert_eq!(items[5].state, State::Ours);
    }

    #[test]
    fn theme_toggle_and_custom_components() {
        let items = scan_str(
            r#"const a = <>
  <Button onClick={() => setTheme("dark")}>tema</Button>
  <Card>sem className fica de fora</Card>
  <Link href="/">link</Link>
</>;"#,
        );
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].category, Category::Theme);
        assert_eq!(items[1].category, Category::Link);
    }
}
