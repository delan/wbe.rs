use std::{fmt::Debug, str};

use egui::{FontId, Rect};

use crate::*;

#[derive(Debug, Clone)]
pub struct PaintText(pub Rect, pub font::FontInfo, pub String);

impl PaintText {
    pub fn rect(&self) -> &Rect {
        &self.0
    }

    pub fn font(&self) -> &FontId {
        &self.1.egui
    }

    pub fn text(&self) -> &str {
        &self.2
    }
}
