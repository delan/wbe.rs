use std::{env::args, fmt::Debug, mem::swap, str};

use egui::{
    pos2, vec2, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, FontId, Frame,
    Layout, Pos2, Rect, TextEdit, Ui, Vec2,
};
use tracing::{debug, error, instrument, trace};

use wbe::*;

// to squelch rust-analyzer error on FONT_PATH in vscode, set
// WBE_FONT_PATH to /dev/null in rust-analyzer.cargo.extraEnv
const HSTEP: f32 = 13.0;
const VSTEP: f32 = 18.0;
const FONT_NAME: &str = "Times New Roman";
const FONT_PATH: &[u8] = include_bytes!(env!("WBE_FONT_PATH"));

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
                .insert(FONT_NAME.to_owned(), FontData::from_static(FONT_PATH));
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
    viewport_rect: Rect,
    scroll: Vec2,
}

impl Default for Browser {
    fn default() -> Self {
        Self {
            tick: Default::default(),
            location: Default::default(),
            document: Default::default(),
            next_document: Default::default(),
            viewport_rect: Rect::NAN,
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
        viewport_rect: Rect,
    },
}

impl Default for Document {
    fn default() -> Self {
        Self::LaidOut {
            location: "about:blank".to_owned(),
            response_body: "".to_owned(),
            display_list: vec![],
            viewport_rect: Rect::NAN,
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
            for PaintText(position, ..) in display_list {
                result = result.max(position.to_vec2());
            }
        }

        result
    }

    fn scroll_limit(&self) -> Vec2 {
        let mut result = self.size();
        if let Self::LaidOut { viewport_rect, .. } = self {
            result -= viewport_rect.size();
        }

        result.max(Vec2::ZERO)
    }
}

#[derive(Debug)]
struct PaintText(Pos2, FontId, String);

impl PaintText {
    fn position(&self) -> &Pos2 {
        &self.0
    }

    fn font(&self) -> &FontId {
        &self.1
    }

    fn text(&self) -> &str {
        &self.2
    }

    fn size(&self) -> Vec2 {
        vec2(HSTEP, VSTEP)
    }

    fn rect(&self) -> Rect {
        Rect::from_min_size(self.0, self.size())
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
        let font = FontId::new(13.0, FontFamily::Proportional);
        let viewport_rect = self.viewport_rect;
        let x_min = viewport_rect.min.x + HSTEP;
        let x_max = viewport_rect.max.x - HSTEP * 2.0;
        let mut cursor = pos2(x_min, viewport_rect.min.y + HSTEP);
        let mut input = response_body.as_bytes();
        let mut display_list = vec![];
        while let Some(token) = lparse_chomp(&mut input, "<.+?>|[^<]+") {
            if !token.starts_with(b"<") {
                for c in str::from_utf8(token).unwrap().chars() {
                    display_list.push(PaintText(cursor, font.clone(), c.to_string()));
                    cursor += vec2(HSTEP, 0.0);
                    if cursor.x > x_max {
                        cursor.x = x_min;
                        cursor.y += VSTEP;
                    }
                }
            }
        }
        debug!(display_item_count = display_list.len());

        Ok(Document::LaidOut {
            location,
            response_body,
            display_list,
            viewport_rect,
        })
    }

    #[instrument(skip(self, ui, display_list))]
    fn paint(&self, ui: &Ui, display_list: &[PaintText], viewport_rect: &Rect) {
        let painter = ui.painter();
        for paint in display_list {
            if paint
                .rect()
                .intersects(viewport_rect.translate(self.scroll))
            {
                painter.text(
                    *paint.position() - self.scroll,
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
        trace!(tick = self.tick);
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
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
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
                    self.viewport_rect = ui.cursor();
                    *self.viewport_rect.bottom_mut() = ctx.screen_rect().bottom();

                    // FIXME minus to work around weird y direction
                    self.scroll -= ui.input(|i| i.scroll_delta);
                    self.scroll = self.scroll.clamp(Vec2::ZERO, self.document.scroll_limit());

                    if let Document::LaidOut {
                        display_list,
                        viewport_rect,
                        ..
                    } = &self.document
                    {
                        self.paint(ui, display_list, viewport_rect);

                        if *viewport_rect != self.viewport_rect {
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
