# alltheani

**all the animations** — encontra todos os pontos de transição de um projeto Next.js e deixa tudo suave, a partir de um painel no terminal.

[Português](#português) · [English](#english)

---

## Português

### Instalar em um comando

Linux e macOS:

```bash
curl -fsSL https://raw.githubusercontent.com/caioribi7/alltheani/main/install.sh | sh
```

Windows (PowerShell):

```powershell
irm https://raw.githubusercontent.com/caioribi7/alltheani/main/install.ps1 | iex
```

Ou, sem instalar nada: `npx alltheani`. Com Rust: `cargo install alltheani`.

Os scripts baixam o binário da última release e conferem o SHA256 antes de instalar.

### Usar

Na raiz do seu projeto Next.js:

```bash
alltheani
```

O painel lista cada ponto de transição com `arquivo:linha` e o estado atual: **sem transição**, **instantâneo**, **animado** (já tinha) ou **alltheani ✓**.

| Tecla | O que faz |
|---|---|
| `tab` / `←` `→` | troca entre categorias e itens |
| `p` / `P` | próximo / anterior preset (na categoria ou só no item) |
| `+` / `-` | duração (100–400 ms) |
| `e` | curva (padrão `cubic-bezier(.2,.8,.2,1)`) |
| `x` | limpa a escolha |
| `s` | **suavizar tudo** com o preset recomendado de cada categoria |
| `a` | mostra o diff e aplica com `y` |
| `u` | desfaz a última aplicação |
| `w` | salva o `alltheani.config.json` |

Sem painel (CI, scripts):

```bash
alltheani scan --json          # lista os pontos
alltheani apply --all          # mostra o diff de "suavizar tudo"
alltheani apply --all --yes    # aplica
alltheani undo                 # desfaz
```

### O que ele encontra

| Categoria | Detecta | Preset recomendado |
|---|---|---|
| Troca de página | `<body>` do `app/layout.tsx` | View Transitions (com fallback) |
| Troca de tema | botões cujo `onClick` mexe no tema | View Transitions |
| Navegação / abas | `<nav>`, `role="tablist"` | indicador deslizante |
| Modais | `<dialog>`, `role="dialog"`, `aria-modal`, `*Modal`, `*DialogContent` | scale |
| Toasts | `role="status"`/`alert`, `aria-live`, `*Toast` | slide |
| Menus | `role="menu"`/`listbox`, `*MenuContent`, `*PopoverContent` | scale |
| Acordeões | `<details>`, `Accordion*`, `Collapsible*` | fade |
| Progresso | `<progress>`, `role="progressbar"` | largura animada |
| Itens de lista | o elemento devolvido por `.map(...)` | fade |
| Botões e links | `<button>`, `<a>`, `<Link>` | press (hover/foco/clique) |
| Hover / foco | qualquer elemento com `hover:`, `focus:`, `active:`… | suave |

A leitura do código usa o parser [oxc](https://oxc.rs) (TSX/JSX). Classes Tailwind (`transition`, `animate-`, `duration-`) e classes do seu CSS que já têm `transition`/`animation` contam como "animado", e o alltheani não mexe nelas a não ser que você peça.

### O que ele escreve

- **Runtime** em `src/alltheani/` (ou `alltheani/`):
  - `alltheani.css`: tokens (`--ata-dur`, `--ata-ease`…) e classes `ata-*`;
  - `transitions.tsx`: `<AniTransitions />` e `withViewTransition()`;
  - `sliding-indicator.tsx`: `<SlidingIndicator />`.
- **Codemods mínimos**: acrescenta classes ao `className` (string, template, `cn()`/`clsx()`, ou `[expr, "ata-x"].join(" ")`), envolve o `onClick` do tema e insere os componentes acima, com os imports.
- **`prefers-reduced-motion`** sempre respeitado: tudo vira instantâneo.
- **`alltheani.config.json`** com as escolhas e **`docs/transicoes.md`** com a lista de todas as transições.
- **Backup** em `.alltheani/backups/` antes de escrever (`alltheani undo` restaura).

### Limites da v0.1

- App Router. No Pages Router, importe o `alltheani.css` no `_app`.
- O indicador deslizante acha o item ativo por `aria-current`, `aria-selected`, `data-state="active"` ou `data-active="true"`. O painel avisa quando não encontra nenhum.
- Transição de página cobre cliques em links. `router.push()` usa o fallback (fade no `<main>`).
- Animações de saída só onde o CSS consegue (`<dialog>`). Contadores animados ficam para a v0.2.
- Componentes próprios sem `className` não são tocados: o elemento nativo dentro deles é encontrado no arquivo do componente.

### Desenvolvimento

```bash
cargo test                   # unitários + ponta a ponta no app de exemplo
cd fixtures/next-app && npm install && npm run build
```

O CI roda os testes em Linux, macOS e Windows, aplica "suavizar tudo" no app de exemplo (Next 16) e confere que o `next build` passa. Uma tag `v*` gera os binários e o `SHA256SUMS` da release.

Faz parte dos projetos abertos do Caio, junto com o [Curso On](https://github.com/caioribi7/curso-on) e o Masterbase.

---

## English

**alltheani** ("all the animations") scans a Next.js project for every transition point and smooths them from a terminal (ratatui) panel.

### Install in one command

```bash
curl -fsSL https://raw.githubusercontent.com/caioribi7/alltheani/main/install.sh | sh   # Linux, macOS
```

```powershell
irm https://raw.githubusercontent.com/caioribi7/alltheani/main/install.ps1 | iex       # Windows
```

Or run it without installing: `npx alltheani`. With Rust: `cargo install alltheani`. The install scripts verify the SHA256 checksum before installing.

### Use

Run `alltheani` in your project root. The panel lists every transition point (buttons, links, nav and tabs active state, page and theme changes, modals, toasts, dropdowns, accordions, progress bars, list items, hover/focus states) with `file:line` and its state: none, instant, animated, or done by alltheani.

Choose a preset per category or per item (fade, slide, scale, spring, press, smooth, sliding indicator, View Transitions, none), plus the duration and easing. Press `s` to smooth everything, then `a` to review the diff before anything is written. `u` undoes the last apply.

Headless: `alltheani scan --json`, `alltheani apply --all [--yes]`, `alltheani undo`.

### What it writes

- A small runtime: `alltheani.css` (motion tokens + `ata-*` classes), `<AniTransitions />` and `withViewTransition()` for App Router View Transitions with a fallback, and `<SlidingIndicator />`.
- Minimal, reviewable codemods: class additions, a wrapped theme `onClick`, inserted components and imports.
- It always respects `prefers-reduced-motion`.
- `alltheani.config.json` keeps your choices, `docs/transicoes.md` lists every transition, and backups go in `.alltheani/` for undo.

See the Portuguese section above for the full detection table and the v0.1 limits.

## License

MIT
