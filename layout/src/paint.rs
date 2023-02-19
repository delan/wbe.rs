use std::{fmt::Debug, str};

use egui::{Color32, FontId, Rect};

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

    pub fn rect_mut(&mut self) -> &mut Rect {
        match self {
            Paint::Text(rect, _, _, _) => rect,
            Paint::Fill(rect, _) => rect,
        }
    }

    pub fn font(&self) -> &FontId {
        match self {
            Paint::Text(_, _, font, _) => &font.egui,
            Paint::Fill(_, _) => todo!(),
        }
    }

    pub fn text(&self) -> &str {
        match self {
            Paint::Text(_, _, _, text) => text,
            Paint::Fill(_, _) => todo!(),
        }
    }

    pub fn fill_color(&self) -> &Color32 {
        match self {
            Paint::Text(_, color, _, _) => todo!(),
            Paint::Fill(_, color) => color,
        }
    }
}
