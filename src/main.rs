use std::{env::args, fmt::Debug, mem::swap, str};

use ab_glyph::{Font, FontRef, PxScaleFont, ScaleFont};
use egui::{
    pos2, vec2, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Layout, Pos2, Rect, Stroke, TextEdit, Ui, Vec2,
};
use tracing::{debug, error, instrument, trace};

use wbe::*;

// to squelch rust-analyzer error on FONT_PATH in vscode, set
// WBE_FONT_PATH to /dev/null in rust-analyzer.cargo.extraEnv
const MARGIN: f32 = 16.0;
const FONT_SIZE: f32 = 16.0;
const FONT_NAME: &str = "Times New Roman";
const FONT_DATA: &[u8] = include_bytes!(env!("WBE_FONT_PATH"));

fn main() -> eyre::Result<()> {
    // log to stdout (level configurable by RUST_LOG=debug)
    tracing_subscriber::fmt::init();

    let location = args()
        .nth(1)
        .unwrap_or("http://example.org/index.html".to_owned());

    let options = eframe::NativeOptions {
        initial_window_size: Some(vec2(800.0, 600.0)),
        ..Default::default()
    };
    eframe::run_native(
        "wbe",
        options,
        Box::new(|cc| {
            let mut font_definitions = FontDefinitions::default();
            font_definitions
                .font_data
                .insert(FONT_NAME.to_owned(), FontData::from_static(FONT_DATA));
            font_definitions
                .families
                .get_mut(&FontFamily::Proportional)
                .unwrap()
                .insert(0, FONT_NAME.to_owned());
            cc.egui_ctx.set_fonts(font_definitions);
            let mut browser = Browser {
                location,
                ..Default::default()
            };
            browser.go();

            Box::new(browser)
        }),
    )
    .unwrap();

    Ok(())
}

struct Browser {
    tick: usize,
    location: String,
    document: Document,
    next_document: Document,
    viewport: ViewportInfo,
    scroll: Vec2,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            tick: Default::default(),
            location: Default::default(),
            document: Default::default(),
            next_document: Default::default(),
            viewport: Default::default(),
            scroll: Vec2::ZERO,
        }
    }
}

#[derive(Debug)]
enum Document {
    None,
    Navigated {
        location: String,
    },
    Loaded {
        location: String,
        response_body: String,
    },
    LaidOut {
        location: String,
        response_body: String,
        display_list: Vec<PaintText>,
        viewport: ViewportInfo,
    },
}

impl Default for Document {
    fn default() -> Self {
        Self::LaidOut {
            location: "about:blank".to_owned(),
            response_body: "".to_owned(),
            display_list: vec![],
            viewport: Default::default(),
        }
    }
}

impl Document {
    fn take(&mut self) -> Self {
        let mut result = Self::None;
        swap(self, &mut result);

        result
    }

    fn invalidate_layout(self) -> Self {
        match self {
            Document::LaidOut {
                location,
                response_body,
                ..
            } => Document::Loaded {
                location,
                response_body,
            },
            other => other,
        }
    }

    fn status(&self) -> &'static str {
        match self {
            Document::None => "None",
            Document::Navigated { .. } => "Navigated",
            Document::Loaded { .. } => "Loaded",
            Document::LaidOut { .. } => "LaidOut",
        }
    }

    fn size(&self) -> Vec2 {
        let mut result = Vec2::ZERO;
        if let Self::LaidOut { display_list, .. } = self {
            for paint in display_list {
                result = result.max(paint.rect().max.to_vec2());
            }
        }

        result
    }

    fn scroll_limit(&self) -> Vec2 {
        let mut result = self.size();
        if let Self::LaidOut { viewport, .. } = self {
            result -= viewport.rect.size();
        }

        result.max(Vec2::ZERO)
    }
}

#[derive(Debug, Clone)]
struct PaintText(Rect, FontInfo, String);

impl PaintText {
    fn rect(&self) -> &Rect {
        &self.0
    }

    fn font(&self) -> &FontId {
        &self.1.egui
    }

    fn text(&self) -> &str {
        &self.2
    }
}

#[derive(Debug, Clone)]
struct FontInfo {
    egui: FontId,
    ab: PxScaleFont<FontRef<'static>>,
}

impl FontInfo {
    #[instrument(skip(data))]
    fn new(
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

#[derive(Debug, PartialEq, Clone)]
struct ViewportInfo {
    rect: Rect,
    scale: f32,
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
    fn update(&mut self, cursor: Rect, screen_rect: Rect, pixels_per_point: f32) -> &mut Self {
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

impl Browser {
    #[instrument(skip(self))]
    fn go(&mut self) {
        let location = self.location.clone();
        self.next_document = Document::Navigated { location };
    }

    #[instrument(skip(self))]
    fn load(&mut self, location: String) -> eyre::Result<Document> {
        let (_headers, body) = http::request(&self.location)?;

        Ok(Document::Loaded {
            location,
            // TODO: hard-coding utf-8 is not correct in practice
            response_body: str::from_utf8(&body)?.to_owned(),
        })
    }

    #[instrument(skip(self, response_body))]
    fn layout(&mut self, location: String, response_body: String) -> eyre::Result<Document> {
        let viewport = self.viewport.clone();
        let mut font = FontInfo::new(
            FontFamily::Proportional,
            FONT_DATA,
            FONT_SIZE,
            viewport.scale,
        )?;
        let mut font2 = FontInfo::new(
            FontFamily::Proportional,
            FONT_DATA,
            FONT_SIZE * 1.25,
            viewport.scale,
        )?;
        let x_min = viewport.rect.min.x + MARGIN;
        let x_max = viewport.rect.max.x - MARGIN;
        let mut cursor = pos2(x_min, viewport.rect.min.y + MARGIN);
        let mut input = &*response_body;
        let mut display_list = Vec::<PaintText>::default();

        // per-line data
        let mut i = 0;
        let mut max_ascent = 0.0f32;
        let mut max_height = 0.0f32;

        while let Some(mut token) = lparse_chomp(&mut input, r"<.+?>|[\t\n\v\r ]+|[^<]+") {
            if !token.starts_with("<") {
                if lparse(token, r"[\t\n\v\r ]+").is_some() {
                    token = " ";
                }
                for c in token.chars() {
                    let glyph_id = font.ab.glyph_id(c);
                    let advance = font.ab.h_advance(glyph_id) / viewport.scale;
                    let ascent = font.ab.ascent() / viewport.scale;
                    let height = font.ab.height() / viewport.scale;
                    if cursor.x + advance > x_max {
                        for paint in &mut display_list[i..] {
                            *paint.0.top_mut() += max_ascent - paint.1.ab.ascent() / viewport.scale;
                        }
                        cursor.x = x_min;
                        cursor.y += max_height;
                        i = display_list.len();
                        max_ascent = 0.0;
                        max_height = 0.0;
                    }
                    max_ascent = max_ascent.max(ascent);
                    max_height = max_height.max(height);
                    let rect = Rect::from_min_size(cursor, vec2(advance, height));
                    display_list.push(PaintText(rect, font.clone(), c.to_string()));
                    cursor.x += advance;
                    swap(&mut font, &mut font2);
                }
            }
        }
        for paint in &mut display_list[i..] {
            *paint.0.top_mut() += max_ascent - paint.1.ab.ascent() / viewport.scale;
        }
        trace!(display_list_len = display_list.len());

        Ok(Document::LaidOut {
            location,
            response_body,
            display_list,
            viewport,
        })
    }

    #[instrument(skip(self, ui, display_list))]
    fn paint(&self, ui: &Ui, display_list: &[PaintText], viewport: &ViewportInfo) {
        let painter = ui.painter();
        for paint in display_list {
            let rect = paint.rect().translate(-self.scroll);
            if rect.intersects(viewport.rect) {
                painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_rgb(255, 0, 255)));
                painter.text(
                    rect.min,
                    Align2::LEFT_TOP,
                    paint.text(),
                    paint.font().clone(),
                    Color32::BLACK,
                );
            }
        }
    }

    #[instrument(skip(self))]
    fn tick(&mut self) -> eyre::Result<()> {
        // trace!(tick = self.tick);
        self.tick += 1;

        // debug!(
        //     document = self.document.status(),
        //     next_document = self.next_document.status(),
        //     size = ?self.document.size(),
        //     scroll_limit = ?self.document.scroll_limit(),
        // );
        self.next_document = match self.next_document.take() {
            Document::None => return Ok(()),
            Document::Navigated { location } => self.load(location)?,
            Document::Loaded {
                location,
                response_body,
            } => self.layout(location, response_body)?,
            document @ Document::LaidOut { .. } => document,
        };
        if let Document::LaidOut { .. } = &self.next_document {
            self.document = self.next_document.take();
        }

        Ok(())
    }
}

impl eframe::App for Browser {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        if let Err(e) = self.tick() {
            error!("error: {}", e.to_string());
            panic!();
        }

        egui::TopBottomPanel::top("location").show(ctx, |ui| {
            ui.allocate_ui_with_layout(
                ui.available_size(),
                Layout::right_to_left(Align::Center),
                |ui| {
                    if ui.button("go").clicked() {
                        self.go();
                    }
                    ui.add_sized(
                        ui.available_size(),
                        TextEdit::singleline(&mut self.location),
                    );
                },
            );
        });

        egui::CentralPanel::default()
            .frame(Frame::none().fill(Color32::WHITE))
            .show(ctx, |ui| {
                // needed only for scroll_delta
                egui::ScrollArea::both().show(ui, |ui| {
                    // FIXME minus to work around weird y direction
                    self.scroll -= ui.input(|i| i.scroll_delta);
                    self.scroll = self.scroll.clamp(Vec2::ZERO, self.document.scroll_limit());

                    if let Document::LaidOut {
                        display_list,
                        viewport,
                        ..
                    } = &self.document
                    {
                        self.paint(ui, display_list, viewport);

                        if viewport
                            != self.viewport.update(
                                ui.cursor(),
                                ctx.screen_rect(),
                                ctx.pixels_per_point(),
                            )
                        {
                            let document = self.document.take().invalidate_layout();
                            if let Document::Loaded {
                                location,
                                response_body,
                            } = document
                            {
                                self.document = self.layout(location, response_body).unwrap();
                            } else {
                                self.document = document;
                            }
                        }
                    }
                });
            });
    }
}
