//! `docs/transicoes.md`: a lista de todas as transições do projeto.

use crate::model::{easing_value, Category, Config, State};
use crate::scan::Project;

pub fn markdown(project: &Project, cfg: &Config) -> String {
    let mut out = String::new();
    out.push_str("# Transições\n\n");
    out.push_str("> Gerado pelo [alltheani](https://github.com/caioribi7/alltheani). Rode `alltheani` na raiz do projeto para mudar e regenerar.\n\n");
    out.push_str(&format!(
        "Padrão: **{} ms**, curva **{}** (`{}`). Com `prefers-reduced-motion: reduce`, tudo vira instantâneo.\n\n",
        cfg.duration,
        cfg.easing,
        easing_value(&cfg.easing)
    ));
    out.push_str(&format!(
        "Runtime em `{rt}/`: `alltheani.css` (tokens e classes `ata-`), `transitions.tsx` (`<AniTransitions />`, `withViewTransition`) e `sliding-indicator.tsx` (`<SlidingIndicator />`).\n\n",
        rt = project.runtime_dir
    ));

    let total = project.items.len();
    let smooth = project
        .items
        .iter()
        .filter(|i| cfg.effective(i).is_some() || i.state != State::Instant && i.state != State::None)
        .count();
    out.push_str(&format!("**{smooth} de {total}** pontos com transição.\n\n"));

    for cat in Category::ALL {
        let items: Vec<_> = project.items.iter().filter(|i| i.category == cat).collect();
        if items.is_empty() {
            continue;
        }
        out.push_str(&format!("## {} ({})\n\n", cat.label(), items.len()));
        out.push_str("| Onde | Elemento | Estado | Transição | Duração | Curva |\n|---|---|---|---|---|---|\n");
        for i in items {
            let (t, d, e) = match cfg.effective(i) {
                Some(eff) => (
                    eff.preset.label().to_string(),
                    format!("{} ms", eff.duration),
                    eff.easing,
                ),
                None => ("—".into(), "—".into(), "—".into()),
            };
            let note = i.note.as_ref().map(|n| format!(" — _{n}_")).unwrap_or_default();
            out.push_str(&format!(
                "| `{}:{}` | `<{}>`{} | {} | {} | {} | {} |\n",
                i.file,
                i.line,
                i.tag,
                note,
                i.state.label(),
                t,
                d,
                e
            ));
        }
        out.push('\n');
    }
    out
}
