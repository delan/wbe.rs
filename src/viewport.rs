use std::fmt::Debug;

use egui::Rect;
use tracing::{debug, instrument};

#[derive(Debug, PartialEq, Clone)]
pub struct ViewportInfo {
    pub rect: Rect,
    pub scale: f32,
}

impl Default for ViewportInfo {
    fn default() -> Self {
        Self {
            rect: Rect::NAN,
            scale: f32::NAN,
        }
    }
}

impl ViewportInfo {
    #[instrument(skip(self, cursor, screen_rect, pixels_per_point))]
    pub fn update(&mut self, cursor: Rect, screen_rect: Rect, pixels_per_point: f32) -> &mut Self {
        // e.g. cursor [[0 24] - [800 inf]], screen_rect [[0 0] - [800 600]]
        let mut viewport_rect = cursor;
        *viewport_rect.bottom_mut() = screen_rect.bottom();

        if viewport_rect != self.rect || pixels_per_point != self.scale {
            debug!(?cursor, ?screen_rect, pixels_per_point);
            self.rect = viewport_rect;
            self.scale = pixels_per_point;
        }

        self
    }
}
