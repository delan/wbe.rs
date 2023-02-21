use std::fmt::Debug;

use egui::{Color32, Rect};

use crate::*;

#[derive(Debug, Clone)]
pub enum Paint {
    Text(Rect, Color32, font::FontInfo, String),
    Fill(Rect, Color32),
}

impl Paint {
    pub fn rect(&self) -> &Rect {
        match self {
            Paint::Text(rect, _, _, _) => rect,
            Paint::Fill(rect, _) => rect,
        }
    }
}
