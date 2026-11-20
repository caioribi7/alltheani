//! Tipos centrais: categorias, presets, estado de cada ponto de transição e a config.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Page,
    Theme,
    Nav,
    Tabs,
    Modal,
    Toast,
    Dropdown,
    Accordion,
    Progress,
    List,
    Button,
    Link,
    Interactive,
}

impl Category {
    pub const ALL: [Category; 13] = [
        Category::Page,
        Category::Theme,
        Category::Nav,
        Category::Tabs,
        Category::Modal,
        Category::Toast,
        Category::Dropdown,
        Category::Accordion,
        Category::Progress,
        Category::List,
        Category::Button,
        Category::Link,
        Category::Interactive,
    ];

    pub fn key(self) -> &'static str {
        match self {
            Category::Page => "page",
            Category::Theme => "theme",
            Category::Nav => "nav",
            Category::Tabs => "tabs",
            Category::Modal => "modal",
            Category::Toast => "toast",
            Category::Dropdown => "dropdown",
            Category::Accordion => "accordion",
            Category::Progress => "progress",
            Category::List => "list",
            Category::Button => "button",
            Category::Link => "link",
            Category::Interactive => "interactive",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Category::Page => "Troca de página",
            Category::Theme => "Troca de tema",
            Category::Nav => "Navegação (ativo)",
            Category::Tabs => "Abas / segmentado",
            Category::Modal => "Modais e diálogos",
            Category::Toast => "Toasts e alertas",
            Category::Dropdown => "Menus e dropdowns",
            Category::Accordion => "Acordeões",
            Category::Progress => "Barras de progresso",
            Category::List => "Itens de lista",
            Category::Button => "Botões",
            Category::Link => "Links",
            Category::Interactive => "Hover / foco",
        }
    }

    /// Presets que fazem sentido para a categoria; o primeiro é o recomendado.
    pub fn presets(self) -> &'static [Preset] {
        use Preset::*;
        match self {
            Category::Page | Category::Theme => &[ViewTransition, None],
            Category::Nav | Category::Tabs => &[Indicator, None],
            Category::Modal | Category::Dropdown => &[Scale, Fade, Slide, Spring, None],
            Category::Toast => &[Slide, Fade, Scale, Spring, None],
            Category::Accordion | Category::List => &[Fade, Slide, Scale, Spring, None],
            Category::Progress => &[Smooth, None],
            Category::Button | Category::Link => &[Press, Smooth, Scale, Spring, None],
            Category::Interactive => &[Smooth, Press, None],
        }
    }

    pub fn recommended(self) -> Preset {
        self.presets()[0]
    }

    pub fn recommended_duration(self) -> u32 {
        match self {
            Category::Button | Category::Link | Category::Interactive => 150,
            Category::Page | Category::Theme | Category::Modal => 250,
            _ => 200,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Preset {
    None,
    Fade,
    Slide,
    Scale,
    Spring,
    Press,
    Smooth,
    Indicator,
    ViewTransition,
}

impl Preset {
    pub fn label(self) -> &'static str {
        match self {
            Preset::None => "nenhuma",
            Preset::Fade => "fade",
            Preset::Slide => "slide",
            Preset::Scale => "scale",
            Preset::Spring => "spring",
            Preset::Press => "press",
            Preset::Smooth => "suave",
            Preset::Indicator => "indicador deslizante",
            Preset::ViewTransition => "View Transitions",
        }
    }

    /// Classe CSS do runtime para presets baseados em classe.
    pub fn class(self, cat: Category) -> Option<&'static str> {
        Some(match self {
            Preset::Fade => "ata-fade",
            Preset::Slide => "ata-slide",
            Preset::Scale => "ata-scale",
            Preset::Spring => "ata-spring",
            Preset::Press => "ata-press",
            Preset::Smooth if cat == Category::Progress => "ata-progress",
            Preset::Smooth => "ata-smooth",
            _ => return Option::None,
        })
    }
}

pub const EASINGS: [(&str, &str); 5] = [
    ("standard", "cubic-bezier(.2,.8,.2,1)"),
    ("out", "cubic-bezier(0,0,.2,1)"),
    ("in-out", "cubic-bezier(.4,0,.2,1)"),
    ("spring", "cubic-bezier(.34,1.56,.64,1)"),
    ("linear", "linear"),
];

pub const DURATIONS: [u32; 8] = [100, 120, 150, 180, 200, 250, 300, 400];

pub fn easing_value(name: &str) -> &'static str {
    EASINGS
        .iter()
        .find(|(n, _)| *n == name)
        .map(|(_, v)| *v)
        .unwrap_or(EASINGS[0].1)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// Nada muda visualmente: sem estados nem transição.
    None,
    /// Muda de estado (hover, ativo, aparece/some) sem transição.
    Instant,
    /// Já tem transição/animação própria.
    Animated,
    /// Já suavizado pelo alltheani (classes `ata-` ou helpers do runtime).
    Ours,
}

impl State {
    pub fn label(self) -> &'static str {
        match self {
            State::None => "sem transição",
            State::Instant => "instantâneo",
            State::Animated => "animado",
            State::Ours => "alltheani ✓",
        }
    }
}

/// Como acrescentar classes ao elemento.
#[derive(Debug, Clone, PartialEq)]
pub enum ClassEdit {
    /// `className="..."` (ou `{"..."}`): offset da aspa de fechamento.
    Str { close: usize, empty: bool },
    /// `className={`...`}`: offset da crase de fechamento.
    Template { close: usize },
    /// `className={cn(...)}`: offset do `)` e o separador a inserir.
    Call { close: usize, sep: &'static str },
    /// Qualquer outra expressão: vira `[expr, "ata-x"].join(" ")`.
    Wrap { start: usize, end: usize },
    /// Sem className: insere o atributo depois do nome da tag.
    Missing { at: usize },
}

#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub file: String,
    pub line: usize,
    pub category: Category,
    pub state: State,
    pub tag: String,
    pub note: Option<String>,
    pub class_edit: Option<ClassEdit>,
    /// Literal (conteúdo sem aspas) que já tem classes `ata-`: (início, fim).
    pub sv_literal: Option<(usize, usize)>,
    /// Fim da tag de abertura (logo após `>`), para inserir um filho.
    pub open_end: Option<usize>,
    /// Indentação da linha da tag de abertura.
    pub indent: String,
    /// Expressão do onClick (início, fim).
    pub on_click: Option<(usize, usize)>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct Choice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub preset: Option<Preset>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub easing: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    /// Duração padrão em ms.
    pub duration: u32,
    /// Nome da curva padrão (ver EASINGS).
    pub easing: String,
    /// Pasta do runtime gerado. Vazio = automático (`src/alltheani` ou `alltheani`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub runtime_dir: String,
    #[serde(default)]
    pub categories: BTreeMap<Category, Choice>,
    #[serde(default)]
    pub items: BTreeMap<String, Choice>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            version: 1,
            duration: 200,
            easing: "standard".into(),
            runtime_dir: String::new(),
            categories: BTreeMap::new(),
            items: BTreeMap::new(),
        }
    }
}

pub const CONFIG_FILE: &str = "alltheani.config.json";

#[derive(Debug, Clone, PartialEq)]
pub struct Effective {
    pub preset: Preset,
    pub duration: u32,
    pub easing: String,
}

impl Config {
    pub fn load(root: &Path) -> anyhow::Result<Config> {
        let p = root.join(CONFIG_FILE);
        if !p.exists() {
            return Ok(Config::default());
        }
        let text = std::fs::read_to_string(&p)?;
        serde_json::from_str(&text).map_err(|e| anyhow::anyhow!("{CONFIG_FILE} inválido: {e}"))
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap() + "\n"
    }

    /// Escolha efetiva de um item: override do item > categoria. `None` = não mexer.
    pub fn effective(&self, item: &Item) -> Option<Effective> {
        let cat = self.categories.get(&item.category);
        let own = self.items.get(&item.id);
        let preset = own.and_then(|c| c.preset).or_else(|| {
            // A escolha da categoria não passa por cima de animações que já existiam.
            if item.state == State::Animated {
                Option::None
            } else {
                cat.and_then(|c| c.preset)
            }
        })?;
        let duration = own
            .and_then(|c| c.duration)
            .or_else(|| cat.and_then(|c| c.duration))
            .unwrap_or(self.duration);
        let easing = own
            .and_then(|c| c.easing.clone())
            .or_else(|| cat.and_then(|c| c.easing.clone()))
            .unwrap_or_else(|| self.easing.clone());
        Some(Effective {
            preset,
            duration,
            easing,
        })
    }

    /// "Suavizar tudo": preset recomendado em todas as categorias.
    pub fn smooth_everything(&mut self) {
        for cat in Category::ALL {
            let c = self.categories.entry(cat).or_default();
            c.preset = Some(cat.recommended());
            if c.duration.is_none() {
                c.duration = Some(cat.recommended_duration());
            }
        }
    }

    /// Classes `ata-` que o elemento deve ter para a escolha dada.
    pub fn classes_for(&self, cat: Category, eff: &Effective) -> Vec<String> {
        let mut out = vec![];
        match eff.preset {
            Preset::Indicator => out.push("ata-indicator-host".to_string()),
            p => {
                if let Some(c) = p.class(cat) {
                    out.push(c.to_string());
                }
            }
        }
        if out.is_empty() {
            return out;
        }
        if eff.duration != self.duration {
            out.push(format!("ata-d-{}", eff.duration));
        }
        if eff.easing != self.easing {
            out.push(format!("ata-e-{}", eff.easing));
        }
        out
    }
}
