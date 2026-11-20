//! Runtime gerado dentro do projeto: tokens de movimento em CSS e helpers React.

use crate::model::{easing_value, Category, Config, DURATIONS, EASINGS};
use std::collections::BTreeSet;

pub const HEADER: &str =
    "Gerado pelo alltheani (https://github.com/caioribi7/alltheani). Rode `alltheani` de novo para atualizar.";

pub fn css(cfg: &Config) -> String {
    let ease = easing_value(&cfg.easing);
    let page = cfg
        .categories
        .get(&Category::Page)
        .and_then(|c| c.duration)
        .unwrap_or(Category::Page.recommended_duration());
    let mut durations: BTreeSet<u32> = DURATIONS.into_iter().collect();
    for c in cfg.categories.values().chain(cfg.items.values()) {
        if let Some(d) = c.duration {
            durations.insert(d);
        }
    }
    let mut out = format!(
        r#"/* {HEADER} */
:root {{
  --ata-dur: {dur}ms;
  --ata-dur-page: {page}ms;
  --ata-ease: {ease};
  --ata-ease-spring: cubic-bezier(.34,1.56,.64,1);
  --ata-distance: 8px;
  --ata-press-scale: .97;
  --ata-indicator-color: currentColor;
  --ata-indicator-size: 2px;
}}

.ata-press, .ata-smooth, .ata-fade, .ata-slide, .ata-scale, .ata-spring {{
  transition-property: color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, opacity, box-shadow, transform, translate, scale;
  transition-duration: var(--ata-dur);
  transition-timing-function: var(--ata-ease);
}}
.ata-press:active:not(:disabled) {{ scale: var(--ata-press-scale); }}

/* Entrada ao montar (modais, toasts, menus, itens de lista). */
.ata-fade {{ animation: ata-fade-in var(--ata-dur) var(--ata-ease) both; }}
.ata-slide {{ animation: ata-slide-in var(--ata-dur) var(--ata-ease) both; }}
.ata-scale {{ animation: ata-scale-in var(--ata-dur) var(--ata-ease) both; transform-origin: top center; }}
.ata-spring {{ animation: ata-scale-in calc(var(--ata-dur) * 1.4) var(--ata-ease-spring) both; }}

/* Barras de progresso: a largura anima quando o valor muda. */
.ata-progress, .ata-progress > * {{
  transition: width var(--ata-dur) var(--ata-ease), inline-size var(--ata-dur) var(--ata-ease), transform var(--ata-dur) var(--ata-ease), scale var(--ata-dur) var(--ata-ease);
}}

/* Indicador deslizante do item ativo (<SlidingIndicator />). */
.ata-indicator-host {{ position: relative; isolation: isolate; }}
.ata-indicator {{
  position: absolute; left: 0; top: 0; z-index: -1;
  height: var(--ata-indicator-size); width: 0;
  background: var(--ata-indicator-color); border-radius: 999px;
  opacity: 0; pointer-events: none;
  transition: translate var(--ata-dur) var(--ata-ease), width var(--ata-dur) var(--ata-ease), opacity var(--ata-dur) var(--ata-ease);
}}
.ata-indicator[data-ready] {{ opacity: 1; }}

/* View Transitions (páginas e tema). */
::view-transition-old(root), ::view-transition-new(root) {{
  animation-duration: var(--ata-dur-page);
  animation-timing-function: var(--ata-ease);
}}

@keyframes ata-fade-in {{ from {{ opacity: 0; }} }}
@keyframes ata-slide-in {{ from {{ opacity: 0; translate: 0 var(--ata-distance); }} }}
@keyframes ata-scale-in {{ from {{ opacity: 0; scale: .96; }} }}

"#,
        dur = cfg.duration,
    );
    for d in durations {
        out.push_str(&format!(".ata-d-{d} {{ --ata-dur: {d}ms; }}\n"));
    }
    for (name, value) in EASINGS {
        out.push_str(&format!(".ata-e-{name} {{ --ata-ease: {value}; }}\n"));
    }
    out.push_str(
        r#"
/* Movimento reduzido: tudo instantâneo, sem deslocamento. */
@media (prefers-reduced-motion: reduce) {
  .ata-press, .ata-smooth, .ata-fade, .ata-slide, .ata-scale, .ata-spring,
  .ata-progress, .ata-progress > *, .ata-indicator {
    transition-duration: 0.01ms !important;
    animation: none !important;
  }
  .ata-press:active:not(:disabled) { scale: none; }
  ::view-transition-group(*), ::view-transition-old(*), ::view-transition-new(*) {
    animation: none !important;
  }
}
"#,
    );
    out
}

pub fn transitions_tsx(cfg: &Config) -> String {
    let page = cfg
        .categories
        .get(&Category::Page)
        .and_then(|c| c.duration)
        .unwrap_or(Category::Page.recommended_duration());
    format!(
        r#"// {HEADER}
"use client";

import {{ useEffect, useRef }} from "react";
import {{ usePathname }} from "next/navigation";
import {{ flushSync }} from "react-dom";

const PAGE_MS = {page};
const EASE = "{ease}";

type ViewTransitionDoc = {{ startViewTransition?: (update: () => void | Promise<void>) => unknown }};

function vtDoc(): ViewTransitionDoc | null {{
  if (typeof document === "undefined") return null;
  if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return null;
  const doc = document as unknown as ViewTransitionDoc;
  return typeof doc.startViewTransition === "function" ? doc : null;
}}

/** Envolve um handler (ex.: trocar o tema) numa View Transition, com fallback. */
export function withViewTransition<A extends unknown[]>(fn: ((...args: A) => unknown) | undefined | null) {{
  return (...args: A) => {{
    if (!fn) return;
    const doc = vtDoc();
    if (!doc?.startViewTransition) {{
      fn(...args);
      return;
    }}
    doc.startViewTransition(() => {{
      flushSync(() => {{
        fn(...args);
      }});
    }});
  }};
}}

/**
 * Transições entre páginas no App Router. Coloque uma vez no <body> do layout raiz.
 * Com suporte a View Transitions: cliques em links internos viram uma transição.
 * Sem suporte: o <main> entra com um fade curto quando a rota muda.
 */
export function AniTransitions() {{
  const pathname = usePathname();
  const pending = useRef<(() => void) | null>(null);
  const first = useRef(true);

  useEffect(() => {{
    function onClick(e: MouseEvent) {{
      if (e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
      const a = (e.target as Element | null)?.closest?.("a[href]") as HTMLAnchorElement | null;
      if (!a || (a.target && a.target !== "_self") || a.hasAttribute("download") || a.dataset.alltheani === "off") return;
      const url = new URL(a.href, location.href);
      if (url.origin !== location.origin || url.pathname === location.pathname) return;
      const doc = vtDoc();
      if (!doc?.startViewTransition) return;
      pending.current?.();
      doc.startViewTransition(
        () =>
          new Promise<void>((resolve) => {{
            pending.current = resolve;
            setTimeout(resolve, 1500);
          }}),
      );
    }}
    document.addEventListener("click", onClick, true);
    return () => document.removeEventListener("click", onClick, true);
  }}, []);

  useEffect(() => {{
    if (first.current) {{
      first.current = false;
      return;
    }}
    const resolve = pending.current;
    if (resolve) {{
      pending.current = null;
      requestAnimationFrame(() => resolve());
      return;
    }}
    if (window.matchMedia("(prefers-reduced-motion: reduce)").matches) return;
    if (vtDoc()) return;
    const el = document.querySelector("main") ?? document.body;
    el.animate?.(
      [
        {{ opacity: 0, translate: "0 6px" }},
        {{ opacity: 1, translate: "0 0" }},
      ],
      {{ duration: PAGE_MS, easing: EASE }},
    );
  }}, [pathname]);

  return null;
}}
"#,
        ease = easing_value(&cfg.easing),
    )
}

pub fn sliding_indicator_tsx() -> String {
    format!(
        r#"// {HEADER}
"use client";

import {{ useEffect, useRef }} from "react";

const ACTIVE =
  '[aria-current="page"],[aria-current="true"],[aria-selected="true"],[data-state="active"],[data-active="true"]';

/**
 * Barra que desliza até o item ativo do elemento pai (nav, tablist...).
 * O pai recebe a classe `ata-indicator-host`; o item ativo é achado por aria-current,
 * aria-selected, data-state="active" ou data-active="true".
 */
export function SlidingIndicator({{ className }}: {{ className?: string }}) {{
  const ref = useRef<HTMLSpanElement>(null);

  useEffect(() => {{
    const bar = ref.current;
    const host = bar?.parentElement;
    if (!bar || !host) return;
    let first = true;

    const place = () => {{
      const active = host.querySelector<HTMLElement>(ACTIVE);
      if (!active) {{
        bar.removeAttribute("data-ready");
        return;
      }}
      const h = host.getBoundingClientRect();
      const a = active.getBoundingClientRect();
      if (first) bar.style.transition = "none";
      bar.style.width = `${{a.width}}px`;
      bar.style.translate = `${{a.left - h.left + host.scrollLeft}}px ${{a.bottom - h.top - bar.offsetHeight}}px`;
      bar.setAttribute("data-ready", "");
      if (first) {{
        void bar.offsetWidth;
        bar.style.transition = "";
        first = false;
      }}
    }};

    place();
    const ro = new ResizeObserver(place);
    ro.observe(host);
    const mo = new MutationObserver(place);
    mo.observe(host, {{
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["aria-current", "aria-selected", "data-state", "data-active"],
    }});
    window.addEventListener("resize", place);
    return () => {{
      ro.disconnect();
      mo.disconnect();
      window.removeEventListener("resize", place);
    }};
  }}, []);

  return <span ref={{ref}} aria-hidden className={{className ? `ata-indicator ${{className}}` : "ata-indicator"}} />;
}}
"#
    )
}
