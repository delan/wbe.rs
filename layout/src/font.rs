use std::fmt::Debug;

use ab_glyph::{Font, FontRef, PxScaleFont};
use egui::{FontFamily, FontId};
use tracing::{instrument, trace};

#[derive(Debug, Clone)]
pub struct FontInfo {
    pub egui: FontId,
    pub ab: PxScaleFont<FontRef<'static>>,
}

impl FontInfo {
    #[instrument(skip(data))]
    pub fn new(
        family: FontFamily,
        data: &'static [u8],
        size_egui_points: f32,
        pixels_per_egui_point: f32,
    ) -> eyre::Result<Self> {
        let font_id = FontId::new(size_egui_points, family);

        let font = FontRef::try_from_slice(data)?;
        let ab_height_unscaled = font.height_unscaled();
        let ab_units_per_em = font.units_per_em().expect("Font::units_per_em() was None");
        let size_pixels =
            size_egui_points * pixels_per_egui_point * ab_height_unscaled / ab_units_per_em;
        trace!(ab_height_unscaled, ab_units_per_em);

        Ok(Self {
            egui: font_id,
            ab: font.into_scaled(size_pixels),
        })
    }
}
