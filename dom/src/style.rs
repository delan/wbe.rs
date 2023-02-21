use egui::Color32;

use wbe_core::FONT_SIZE;

lazy_static::lazy_static! {
    pub static ref INITIAL_STYLE: Style = Style {
        display: Some("inline".to_owned()),
        font_size: Some(FONT_SIZE),
        font_weight: Some(CssFontWeight::Normal),
        font_style: Some(CssFontStyle::Normal),
        width: Some(CssWidth::Auto),
        background_color: Some(Color32::TRANSPARENT),
        color: Some(Color32::BLACK),
    };
}

#[derive(Debug, Clone)]
pub struct Style {
    pub display: Option<String>,
    pub font_size: Option<f32>,
    pub font_weight: Option<CssFontWeight>,
    pub font_style: Option<CssFontStyle>,
    pub width: Option<CssWidth>,
    pub background_color: Option<Color32>,
    pub color: Option<Color32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssDisplay {
    None,
    Inline,
    Block,
    InlineBlock,
    ListItem,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssFontWeight {
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssFontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssWidth {
    Auto,
    Length(CssLength),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssLength {
    Percent(f32),
    Px(f32),
    Em(f32),
}

impl Style {
    pub fn empty() -> Self {
        Self {
            display: None,
            font_size: None,
            font_weight: None,
            font_style: None,
            width: None,
            background_color: None,
            color: None,
        }
    }

    pub fn initial() -> &'static Self {
        &*INITIAL_STYLE
    }

    pub fn new_inherited(&self) -> Self {
        Self {
            color: self.color.clone(),
            ..Self::initial().clone()
        }
    }

    pub fn apply(&mut self, other: &Style) {
        self.display = other.display.clone().or(self.display.clone());
        self.background_color = other
            .background_color
            .clone()
            .or(self.background_color.clone());
        self.color = other.color.clone().or(self.color.clone());
    }

    pub fn display(&self) -> CssDisplay {
        match &**self
            .display
            .as_ref()
            .unwrap_or_else(|| Self::initial().display.as_ref().unwrap())
        {
            "none" => CssDisplay::None,
            "inline" => CssDisplay::Inline,
            "block" => CssDisplay::Block,
            _ => CssDisplay::Inline,
        }
    }

    pub fn font_size(&self) -> f32 {
        self.get(|s| s.font_size)
    }

    pub fn font_weight(&self) -> CssFontWeight {
        self.get(|s| s.font_weight)
    }

    pub fn font_style(&self) -> CssFontStyle {
        self.get(|s| s.font_style)
    }

    pub fn width(&self, percent_base: f32, em_base: f32) -> f32 {
        resolve_width(self.get(|s| s.width), percent_base, em_base)
    }

    pub fn background_color(&self) -> Color32 {
        self.get(|s| s.background_color)
    }

    pub fn color(&self) -> Color32 {
        self.get(|s| s.color)
    }

    fn get<T>(&self, getter: impl Fn(&Self) -> Option<T>) -> T {
        getter(self).unwrap_or_else(|| getter(Self::initial()).unwrap())
    }
}

pub fn resolve_length(value: CssLength, percent_base: f32, em_base: f32) -> f32 {
    match value {
        CssLength::Percent(x) => x / 100.0 * percent_base,
        CssLength::Px(x) => x,
        CssLength::Em(x) => x * em_base,
    }
}

pub fn resolve_width(value: CssWidth, percent_base: f32, em_base: f32) -> f32 {
    match value {
        CssWidth::Auto => percent_base,
        CssWidth::Length(x) => resolve_length(x, percent_base, em_base),
    }
}
