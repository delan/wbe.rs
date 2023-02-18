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
    pub fn is_valid(&self) -> bool {
        return !self.rect.any_nan() && !self.scale.is_nan();
    }

    #[instrument(skip(self, viewport_rect, pixels_per_point))]
    pub fn update(&mut self, viewport_rect: Rect, pixels_per_point: f32) -> &mut Self {
        if viewport_rect != self.rect || pixels_per_point != self.scale {
            debug!(?viewport_rect, pixels_per_point);
            self.rect = viewport_rect;
            self.scale = pixels_per_point;
        }

        self
    }
}
