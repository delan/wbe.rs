use egui::Color32;
use tracing::error;

#[derive(Debug, Default, Clone)]
pub struct Style {
    pub background_color: Option<String>,
    pub color: Option<String>,
}

impl Style {
    pub fn new_inherited(&self) -> Self {
        Self {
            color: self.color.clone(),
            ..Default::default()
        }
    }

    pub fn apply(&mut self, other: &Style) {
        self.background_color = other
            .background_color
            .clone()
            .or(self.background_color.clone());
        self.color = other.color.clone().or(self.color.clone());
    }

    pub fn get_background_color(&self) -> Color32 {
        get_color(self.background_color.as_deref().unwrap_or("transparent"))
    }

    pub fn get_color(&self) -> Color32 {
        get_color(self.color.as_deref().unwrap_or("black"))
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
