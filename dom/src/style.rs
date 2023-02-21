use egui::Color32;
use tracing::error;

lazy_static::lazy_static! {
    pub static ref INITIAL_STYLE: Style = Style {
        display: Some("inline".to_owned()),
        background_color: Some("transparent".to_owned()),
        color: Some("black".to_owned()),
    };
}

#[derive(Debug, Clone)]
pub struct Style {
    pub display: Option<String>,
    pub background_color: Option<String>,
    pub color: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssDisplay {
    None,
    Inline,
    Block,
    InlineBlock,
    ListItem,
}

impl Style {
    pub fn empty() -> Self {
        Self {
            display: None,
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

    pub fn background_color(&self) -> Color32 {
        get_color(
            self.background_color
                .as_ref()
                .unwrap_or_else(|| Self::initial().background_color.as_ref().unwrap()),
        )
    }

    pub fn color(&self) -> Color32 {
        get_color(
            self.color
                .as_ref()
                .unwrap_or_else(|| Self::initial().color.as_ref().unwrap()),
        )
    }
}

fn get_color(color: &str) -> Color32 {
    match color {
        "transparent" => Color32::TRANSPARENT,
        "blue" => Color32::BLUE,
        "white" => Color32::WHITE,
        "black" => Color32::BLACK,
        "rgb(204,0,0)" => Color32::from_rgb(204, 0, 0),
        "#FC0" => Color32::from_rgb(0xFF, 0xCC, 0x00),
        "#663399" => Color32::from_rgb(0x66, 0x33, 0x99),
        "#008080" => Color32::from_rgb(0x00, 0x80, 0x80),
        other => {
            error!("unknown color {:?}", other);
            Color32::TEMPORARY_COLOR
        }
    }
}
