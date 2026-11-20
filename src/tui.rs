//! Painel no terminal (ratatui).

use crate::codemod::{self, Change};
use crate::model::{Category, Choice, Config, Item, Preset, State, DURATIONS, EASINGS};
use crate::scan::{self, Project};
use crate::{fsops, model};
use ratatui::crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, BorderType, Cell, List, ListItem, ListState, Paragraph, Row, Table, TableState};
use ratatui::Frame;
use std::path::{Path, PathBuf};

const BRAND: Color = Color::Rgb(124, 92, 255);
const MINT: Color = Color::Rgb(29, 178, 122);
const SUN: Color = Color::Rgb(255, 200, 61);
const CORAL: Color = Color::Rgb(255, 107, 74);
const MUTED: Color = Color::Rgb(138, 143, 156);

#[derive(PartialEq, Clone, Copy)]
enum Focus {
    Cats,
    Items,
}

struct Diff {
    changes: Vec<Change>,
    lines: Vec<Line<'static>>,
    scroll: u16,
}

struct App {
    root: PathBuf,
    cfg: Config,
    project: Project,
    cats: Vec<Category>,
    cat: usize,
    item: usize,
    focus: Focus,
    diff: Option<Diff>,
    status: String,
    dirty: bool,
    quit_armed: bool,
}

pub fn run(root: &Path) -> anyhow::Result<()> {
    let cfg = Config::load(root)?;
    let project = scan::scan(root, &cfg.runtime_dir)?;
    let mut app = App {
        root: project.root.clone(),
        cfg,
        project,
        cats: vec![],
        cat: 0,
        item: 0,
        focus: Focus::Cats,
        diff: None,
        status: String::new(),
        dirty: false,
        quit_armed: false,
    };
    app.refresh_cats();
    app.status = match app.project.warnings.first() {
        Some(w) => format!("! {w}"),
        None => format!(
            "{} pontos encontrados. Aperte s para suavizar tudo.",
            app.project.items.len()
        ),
    };

    let mut term = ratatui::init();
    let res = (|| -> anyhow::Result<()> {
        loop {
            term.draw(|f| draw(f, &mut app))?;
            if let Event::Key(k) = event::read()? {
                if k.kind != KeyEventKind::Press {
                    continue;
                }
                if k.code == KeyCode::Char('c') && k.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }
                if !app.key(k.code)? {
                    return Ok(());
                }
            }
        }
    })();
    ratatui::restore();
    res
}

impl App {
    fn refresh_cats(&mut self) {
        self.cats = Category::ALL
            .into_iter()
            .filter(|c| self.project.items.iter().any(|i| i.category == *c))
            .collect();
        self.cat = self.cat.min(self.cats.len().saturating_sub(1));
        self.item = self.item.min(self.items().len().saturating_sub(1));
    }

    fn rescan(&mut self) -> anyhow::Result<()> {
        self.project = scan::scan(&self.root, &self.cfg.runtime_dir)?;
        self.refresh_cats();
        Ok(())
    }

    fn current_cat(&self) -> Option<Category> {
        self.cats.get(self.cat).copied()
    }

    fn items(&self) -> Vec<&Item> {
        match self.current_cat() {
            Some(c) => self.project.items.iter().filter(|i| i.category == c).collect(),
            None => vec![],
        }
    }

    /// Escolha sendo editada: a da categoria ou a do item selecionado.
    fn target(&mut self) -> Option<(&mut Choice, Category, Option<String>)> {
        let cat = self.current_cat()?;
        if self.focus == Focus::Items {
            let id = self.items().get(self.item)?.id.clone();
            Some((self.cfg.items.entry(id.clone()).or_default(), cat, Some(id)))
        } else {
            Some((self.cfg.categories.entry(cat).or_default(), cat, None))
        }
    }

    fn cleanup(&mut self) {
        self.cfg.items.retain(|_, c| *c != Choice::default());
        self.cfg.categories.retain(|_, c| *c != Choice::default());
        self.dirty = true;
    }

    fn cycle_preset(&mut self, back: bool) {
        let default_dur = self.cfg.duration;
        let Some((choice, cat, _)) = self.target() else { return };
        let mut opts: Vec<Option<Preset>> = vec![None];
        opts.extend(cat.presets().iter().map(|p| Some(*p)));
        let i = opts.iter().position(|o| *o == choice.preset).unwrap_or(0);
        let n = opts.len();
        choice.preset = opts[if back { (i + n - 1) % n } else { (i + 1) % n }];
        if choice.preset.is_some() && choice.duration.is_none() && cat.recommended_duration() != default_dur {
            choice.duration = Some(cat.recommended_duration());
        }
        self.cleanup();
    }

    fn cycle_duration(&mut self, up: bool) {
        let fallback = self.cfg.duration;
        let Some((choice, _, _)) = self.target() else { return };
        let cur = choice.duration.unwrap_or(fallback);
        let i = DURATIONS.iter().position(|d| *d >= cur).unwrap_or(DURATIONS.len() - 1);
        let j = if up {
            (i + 1).min(DURATIONS.len() - 1)
        } else {
            i.saturating_sub(1)
        };
        choice.duration = Some(DURATIONS[j]);
        self.cleanup();
    }

    fn cycle_easing(&mut self) {
        let fallback = self.cfg.easing.clone();
        let Some((choice, _, _)) = self.target() else { return };
        let cur = choice.easing.clone().unwrap_or(fallback);
        let i = EASINGS.iter().position(|(n, _)| *n == cur).unwrap_or(0);
        choice.easing = Some(EASINGS[(i + 1) % EASINGS.len()].0.to_string());
        self.cleanup();
    }

    fn clear(&mut self) {
        if let Some((choice, _, _)) = self.target() {
            *choice = Choice::default();
        }
        self.cleanup();
    }

    fn open_diff(&mut self) -> anyhow::Result<()> {
        let changes = codemod::plan(&self.project, &self.cfg)?;
        if changes.is_empty() {
            self.status = "Nada a mudar: escolha transições (p) ou aperte s.".into();
            return Ok(());
        }
        let mut lines = vec![];
        for c in &changes {
            let tag = if c.old.is_none() { "novo" } else { "alterado" };
            lines.push(Line::from(vec![
                Span::styled(format!(" {} ", c.path), Style::new().bg(BRAND).fg(Color::White).bold()),
                Span::styled(format!(" {tag}"), Style::new().fg(MUTED)),
            ]));
            // Arquivos gerados grandes ficam resumidos; o código do usuário aparece inteiro.
            let text = codemod::diff(c);
            let long = !c.is_source() && text.lines().count() > 40;
            for l in text.lines().skip(2).take(if long { 12 } else { usize::MAX }) {
                let style = if l.starts_with('+') {
                    Style::new().fg(MINT)
                } else if l.starts_with('-') {
                    Style::new().fg(CORAL)
                } else if l.starts_with("@@") {
                    Style::new().fg(BRAND)
                } else {
                    Style::new().fg(MUTED)
                };
                lines.push(Line::styled(l.to_string(), style));
            }
            if long {
                lines.push(Line::styled(
                    format!("  … mais {} linhas", text.lines().count() - 14),
                    Style::new().fg(MUTED).italic(),
                ));
            }
            lines.push(Line::raw(""));
        }
        self.diff = Some(Diff {
            changes,
            lines,
            scroll: 0,
        });
        Ok(())
    }

    /// Devolve false para sair.
    fn key(&mut self, code: KeyCode) -> anyhow::Result<bool> {
        if let Some(d) = &mut self.diff {
            match code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('n') => self.diff = None,
                KeyCode::Down | KeyCode::Char('j') => d.scroll = d.scroll.saturating_add(1),
                KeyCode::Up | KeyCode::Char('k') => d.scroll = d.scroll.saturating_sub(1),
                KeyCode::PageDown | KeyCode::Char(' ') => d.scroll = d.scroll.saturating_add(20),
                KeyCode::PageUp => d.scroll = d.scroll.saturating_sub(20),
                KeyCode::Enter | KeyCode::Char('y') => {
                    let changes = std::mem::take(&mut d.changes);
                    self.diff = None;
                    let n = changes.len();
                    let dir = codemod::write_all(&self.project, &self.cfg, changes)?;
                    self.dirty = false;
                    self.rescan()?;
                    self.status = format!(
                        "✓ {} arquivo(s) escritos. Backup em {} — u desfaz.",
                        n,
                        scan::rel(&self.root, &dir)
                    );
                }
                _ => {}
            }
            return Ok(true);
        }

        if code != KeyCode::Char('q') && code != KeyCode::Esc {
            self.quit_armed = false;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => {
                if self.dirty && !self.quit_armed {
                    self.quit_armed = true;
                    self.status = "Escolhas não salvas. w salva, a aplica, q de novo sai.".into();
                } else {
                    return Ok(false);
                }
            }
            KeyCode::Tab | KeyCode::BackTab => {
                self.focus = if self.focus == Focus::Cats {
                    Focus::Items
                } else {
                    Focus::Cats
                };
            }
            KeyCode::Right | KeyCode::Char('l') if self.focus == Focus::Cats => self.focus = Focus::Items,
            KeyCode::Left | KeyCode::Char('h') if self.focus == Focus::Items => self.focus = Focus::Cats,
            KeyCode::Down | KeyCode::Char('j') => match self.focus {
                Focus::Cats => {
                    self.cat = (self.cat + 1).min(self.cats.len().saturating_sub(1));
                    self.item = 0;
                }
                Focus::Items => self.item = (self.item + 1).min(self.items().len().saturating_sub(1)),
            },
            KeyCode::Up | KeyCode::Char('k') => match self.focus {
                Focus::Cats => {
                    self.cat = self.cat.saturating_sub(1);
                    self.item = 0;
                }
                Focus::Items => self.item = self.item.saturating_sub(1),
            },
            KeyCode::Char('p') | KeyCode::Enter | KeyCode::Char(' ') => self.cycle_preset(false),
            KeyCode::Char('P') => self.cycle_preset(true),
            KeyCode::Char('+') | KeyCode::Char('=') => self.cycle_duration(true),
            KeyCode::Char('-') => self.cycle_duration(false),
            KeyCode::Char('e') => self.cycle_easing(),
            KeyCode::Char('x') => self.clear(),
            KeyCode::Char('s') => {
                self.cfg.smooth_everything();
                self.dirty = true;
                self.status = "Tudo com o preset recomendado. a mostra o diff antes de escrever.".into();
            }
            KeyCode::Char('a') => self.open_diff()?,
            KeyCode::Char('w') => {
                std::fs::write(self.root.join(model::CONFIG_FILE), self.cfg.to_json())?;
                self.dirty = false;
                self.status = format!("✓ {} salvo.", model::CONFIG_FILE);
            }
            KeyCode::Char('u') => match fsops::undo(&self.root) {
                Ok(n) => {
                    self.cfg = Config::load(&self.root)?;
                    self.rescan()?;
                    self.status = format!("↺ {n} arquivo(s) restaurado(s).");
                }
                Err(e) => self.status = e.to_string(),
            },
            KeyCode::Char('r') => {
                self.rescan()?;
                self.status = format!("{} pontos encontrados.", self.project.items.len());
            }
            _ => {}
        }
        Ok(true)
    }
}

fn glyph(c: char) -> [&'static str; 5] {
    match c {
        'A' => [" ██ ", "█  █", "████", "█  █", "█  █"],
        'L' => ["█   ", "█   ", "█   ", "█   ", "████"],
        'T' => ["████", " ██ ", " ██ ", " ██ ", " ██ "],
        'H' => ["█  █", "█  █", "████", "█  █", "█  █"],
        'E' => ["████", "█   ", "███ ", "█   ", "████"],
        'N' => ["█  █", "██ █", "█ ██", "█  █", "█  █"],
        'I' => ["███ ", " █  ", " █  ", " █  ", "███ "],
        _ => ["    "; 5],
    }
}

fn logo() -> Text<'static> {
    // ALL · THE · ANI, cada palavra numa cor.
    let colors = [BRAND, BRAND, BRAND, MINT, MINT, MINT, SUN, SUN, SUN];
    let lines = (0..5)
        .map(|row| {
            Line::from(
                "ALLTHEANI"
                    .chars()
                    .zip(colors)
                    .map(|(c, color)| Span::styled(format!("{} ", glyph(c)[row]), Style::new().fg(color).bold()))
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    Text::from(lines)
}

fn state_style(s: State) -> Style {
    match s {
        State::None => Style::new().fg(MUTED),
        State::Instant => Style::new().fg(SUN),
        State::Animated => Style::new().fg(Color::Cyan),
        State::Ours => Style::new().fg(MINT).bold(),
    }
}

fn panel(title: &str, active: bool) -> Block<'_> {
    Block::bordered()
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(if active { BRAND } else { Color::DarkGray }))
        .title(Span::styled(
            format!(" {title} "),
            Style::new().fg(if active { Color::White } else { MUTED }).bold(),
        ))
}

fn choice_label(cfg: &Config, c: Option<&Choice>) -> String {
    match c.and_then(|c| c.preset) {
        None => "—".into(),
        Some(p) => {
            let c = c.unwrap();
            format!(
                "{} · {}ms · {}",
                p.label(),
                c.duration.unwrap_or(cfg.duration),
                c.easing.clone().unwrap_or(cfg.easing.clone())
            )
        }
    }
}

fn draw(f: &mut Frame, app: &mut App) {
    let [header, body, status, footer] = Layout::vertical([
        Constraint::Length(7),
        Constraint::Min(6),
        Constraint::Length(1),
        Constraint::Length(1),
    ])
    .areas(f.area());

    let [logo_area, tag] = Layout::vertical([Constraint::Length(5), Constraint::Length(1)]).areas(Rect {
        x: header.x + 2,
        y: header.y + 1,
        width: header.width.saturating_sub(4),
        height: 6,
    });
    f.render_widget(Paragraph::new(logo()), logo_area);
    let total = app.project.items.len();
    let todo = app
        .project
        .items
        .iter()
        .filter(|i| matches!(i.state, State::None | State::Instant))
        .count();
    f.render_widget(
        Line::from(vec![
            Span::styled("transições suaves para Next.js", Style::new().fg(Color::White).bold()),
            Span::styled(
                format!(
                    "  ·  {}  ·  ",
                    app.root
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_default()
                ),
                Style::new().fg(MUTED),
            ),
            Span::styled(format!("{total} pontos, {todo} sem transição"), Style::new().fg(SUN)),
        ]),
        tag,
    );

    if let Some(d) = &app.diff {
        let title = format!("Prévia — {} arquivo(s)", d.changes.len());
        let p = Paragraph::new(d.lines.clone())
            .block(panel(&title, true))
            .scroll((d.scroll, 0));
        f.render_widget(p, body);
        f.render_widget(
            Line::styled(
                " Revise antes de escrever. Um backup é salvo em .alltheani/",
                Style::new().fg(MUTED),
            ),
            status,
        );
        f.render_widget(
            keys(&[("y", "aplicar"), ("↑↓ PgUp PgDn", "rolar"), ("esc", "voltar")]),
            footer,
        );
        return;
    }

    let [left, right] = Layout::horizontal([Constraint::Length(44), Constraint::Min(30)]).areas(body);

    let cat_items: Vec<ListItem> = app
        .cats
        .iter()
        .map(|c| {
            let items: Vec<_> = app.project.items.iter().filter(|i| i.category == *c).collect();
            let todo = items
                .iter()
                .filter(|i| matches!(i.state, State::None | State::Instant))
                .count();
            let choice = choice_label(&app.cfg, app.cfg.categories.get(c));
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(format!("{:<22}", c.label()), Style::new().fg(Color::White).bold()),
                    Span::styled(format!("{:>3}", items.len()), Style::new().fg(MUTED)),
                    if todo > 0 {
                        Span::styled(format!("  {todo} a suavizar"), Style::new().fg(SUN))
                    } else {
                        Span::styled("  ok", Style::new().fg(MINT))
                    },
                ]),
                Line::styled(
                    format!("  → {choice}"),
                    Style::new().fg(if choice == "—" { MUTED } else { MINT }),
                ),
            ])
        })
        .collect();
    let mut ls = ListState::default().with_selected(Some(app.cat));
    f.render_stateful_widget(
        List::new(cat_items)
            .block(panel("Categorias", app.focus == Focus::Cats))
            .highlight_style(Style::new().bg(Color::Rgb(40, 36, 70)))
            .highlight_symbol("▌"),
        left,
        &mut ls,
    );

    let items = app.items();
    let rows: Vec<Row> = items
        .iter()
        .map(|i| {
            let eff = app.cfg.effective(i);
            let own = app.cfg.items.contains_key(&i.id);
            let t = match &eff {
                Some(e) => format!("{}{} {}ms", if own { "• " } else { "" }, e.preset.label(), e.duration),
                None => "—".into(),
            };
            let mut where_ = Line::from(vec![
                Span::styled(format!("{}:{}", i.file, i.line), Style::new().fg(Color::White)),
                Span::styled(format!("  <{}>", i.tag), Style::new().fg(MUTED)),
            ]);
            if i.note.is_some() {
                where_.push_span(Span::styled("  ⚠", Style::new().fg(SUN)));
            }
            Row::new(vec![
                Cell::from(Span::styled(i.state.label(), state_style(i.state))),
                Cell::from(where_),
                Cell::from(Span::styled(
                    t,
                    Style::new().fg(if eff.is_some() { MINT } else { MUTED }),
                )),
            ])
        })
        .collect();
    let mut ts = TableState::default().with_selected(if app.focus == Focus::Items {
        Some(app.item)
    } else {
        None
    });
    let title = app.current_cat().map(|c| c.label()).unwrap_or("Nada encontrado");
    f.render_stateful_widget(
        Table::new(
            rows,
            [Constraint::Length(14), Constraint::Min(20), Constraint::Length(30)],
        )
        .header(
            Row::new(["estado", "onde", "transição"]).style(Style::new().fg(MUTED).add_modifier(Modifier::UNDERLINED)),
        )
        .block(panel(title, app.focus == Focus::Items))
        .row_highlight_style(Style::new().bg(Color::Rgb(40, 36, 70))),
        right,
        &mut ts,
    );

    let note = if app.focus == Focus::Items {
        items.get(app.item).and_then(|i| i.note.clone())
    } else {
        None
    };
    let msg = note.map(|n| format!("⚠ {n}")).unwrap_or(app.status.clone());
    f.render_widget(Line::styled(format!(" {msg}"), Style::new().fg(MUTED)), status);
    f.render_widget(
        keys(&[
            ("tab", "painel"),
            ("p", "preset"),
            ("+/-", "duração"),
            ("e", "curva"),
            ("x", "limpar"),
            ("s", "suavizar tudo"),
            ("a", "aplicar"),
            ("u", "desfazer"),
            ("w", "salvar"),
            ("q", "sair"),
        ]),
        footer,
    );
}

fn keys(list: &[(&str, &str)]) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for (k, label) in list {
        spans.push(Span::styled(
            format!(" {k} "),
            Style::new().bg(Color::Rgb(50, 50, 60)).fg(Color::White).bold(),
        ));
        spans.push(Span::styled(format!(" {label}  "), Style::new().fg(MUTED)));
    }
    Line::from(spans)
}
