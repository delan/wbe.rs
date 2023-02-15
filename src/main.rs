use std::time::Instant;
use std::{env::args, mem::swap, str};

use ab_glyph::ScaleFont;
use egui::{
    pos2, vec2, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, Frame, Layout, Rect,
    TextEdit, Ui, Vec2,
};
use eyre::bail;
use tracing::{debug, error, info, instrument, trace};

use wbe::document::Document;
use wbe::dom::{Node, NodeData};
use wbe::font::FontInfo;
use wbe::paint::PaintText;
use wbe::parse::{html_token, HtmlToken};
use wbe::viewport::ViewportInfo;
use wbe::*;

// to squelch rust-analyzer error on FONT_PATH in vscode, set
// WBE_FONT_PATH to /dev/null in rust-analyzer.cargo.extraEnv
const MARGIN: f32 = 16.0;
const FONT_SIZE: f32 = 16.0;
const FONT_NAME: &str = "Times New Roman";
const FONT_DATA: &[u8] = include_bytes!(env!("WBE_FONT_PATH"));

// ([if the child is one of these], [the stack must not end with this sequence])
const NO_NEST: &[(&[&str], &[&str])] = &[
    (
        &["p", "table", "form", "h1", "h2", "h3", "h4", "h5", "h6"],
        &["p"],
    ),
    (&["li"], &["li"]),
    (&["dt", "dd"], &["dt"]),
    (&["dt", "dd"], &["dd"]),
    (&["tr"], &["tr"]),
    (&["tr"], &["tr", "td"]),
    (&["tr"], &["tr", "th"]),
    (&["td", "th"], &["td"]),
    (&["td", "th"], &["th"]),
];
const SELF_CLOSING: &[&str] = &[
    "!doctype", "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta",
    "param", "source", "track", "wbr",
];

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
    fn parse(&mut self, location: String, response_body: String) -> eyre::Result<Document> {
        let mut parent = Node::new(NodeData::Document);
        let mut stack = vec![parent.clone()];
        let mut names_stack: Vec<&str> = vec![];
        let mut input = &*response_body;

        while !input.is_empty() {
            let (rest, token) = match html_token(input) {
                Ok(result) => result,
                Err(nom::Err::Incomplete(_)) => ("", HtmlToken::Text(input)),
                Err(e) => bail!("{}; input={:?}", e, input),
            };
            match token {
                HtmlToken::Comment(text) => {
                    parent.append(&[Node::comment(text.to_owned())]);
                }
                HtmlToken::Script(attrs, text) => {
                    // TODO attrs
                    parent.append(&[Node::element("script".to_owned(), vec![])
                        .append(&[Node::text(text.to_owned())])]);
                }
                HtmlToken::Style(attrs, text) => {
                    // TODO attrs
                    parent.append(&[Node::element("style".to_owned(), vec![])
                        .append(&[Node::text(text.to_owned())])]);
                }
                HtmlToken::Tag(true, name, attrs) => {
                    if let Some((i, _)) = names_stack
                        .iter()
                        .enumerate()
                        .rfind(|(_, x)| x.eq_ignore_ascii_case(name))
                    {
                        for _ in 0..(names_stack.len() - i) {
                            let _ = stack.pop().unwrap();
                            let _ = names_stack.pop().unwrap();
                            parent = parent.parent().unwrap();
                        }
                    } else {
                        error!("failed to find match for closing tag: {:?}", name);
                    }
                }
                HtmlToken::Tag(false, name, attrs) => {
                    let element = Node::element(name.to_owned(), vec![]);

                    for &(child_names, suffix) in NO_NEST {
                        if child_names.contains(&&*name) {
                            if names_stack.ends_with(suffix) {
                                trace!(true, name, ?child_names, ?suffix, ?names_stack);
                                for _ in 0..suffix.len() {
                                    let _ = stack.pop().unwrap();
                                    let _ = names_stack.pop().unwrap();
                                    parent = parent.parent().unwrap();
                                }
                            }
                        }
                    }

                    parent.append(&[element.clone()]);

                    if !SELF_CLOSING.contains(&&*name) {
                        stack.push(element.clone());
                        names_stack.push(name);
                        parent = element;
                    }
                }
                HtmlToken::Text(text) => {
                    parent.append(&[Node::text(text.to_owned())]);
                }
            }
            input = rest;
        }

        let dom = stack[0].clone();
        debug!(%dom);

        Ok(Document::Parsed {
            location,
            response_body,
            dom,
        })
    }

    #[instrument(skip(self, response_body, dom))]
    fn layout(
        &mut self,
        location: String,
        response_body: String,
        dom: Node,
    ) -> eyre::Result<Document> {
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
        let mut display_list = Vec::<PaintText>::default();

        // per-line data
        let mut i = 0;
        let mut max_ascent = 0.0f32;
        let mut max_height = 0.0f32;

        let mut parent = dom.clone();
        let mut children = parent.children();
        let mut stack = vec![];
        let mut j = 0;
        while j < children.len() {
            trace!(parent = %parent.data(), child = %children[j].data());
            let descended = match &*children[j].name() {
                "#text" => {
                    let value = children[j].value().unwrap();
                    let mut value = &*value;
                    while let Some(token) = lparse_chomp(&mut value, r"[\t\n\v\r ]+|.") {
                        let mut token = token.get(0).unwrap().as_str();
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
                                    *paint.0.top_mut() +=
                                        max_ascent - paint.1.ab.ascent() / viewport.scale;
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
                            swap(&mut font, &mut font2);
                        }
                    }

                    false
                }
                "#document" => unreachable!(),
                "#comment" => false,
                _other => {
                    let mut pushed = false;
                    if j + 1 < children.len() {
                        stack.push((parent.clone(), j + 1));
                        pushed = true;
                    }
                    parent = children[j].clone();
                    children = parent.children();
                    j = 0;
                    trace!(new_parent_down = %parent.data(), pushed);

                    true
                }
            };
            if !descended {
                j += 1;
            }
            if j >= children.len() {
                if let Some(previous) = stack.pop() {
                    parent = previous.0;
                    children = parent.children();
                    j = previous.1;
                    trace!(new_parent_up = %parent.data());
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
            dom,
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
                // painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_rgb(255, 0, 255)));
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
        let start = Instant::now();
        self.next_document = match self.next_document.take() {
            Document::None => return Ok(()),
            Document::Navigated { location } => self.load(location)?,
            Document::Loaded {
                location,
                response_body,
            } => self.parse(location, response_body)?,
            Document::Parsed {
                location,
                response_body,
                dom,
            } => self.layout(location, response_body, dom)?,
            document @ Document::LaidOut { .. } => document,
        };

        let now = Instant::now();
        info!(status = self.next_document.status(), duration = ?now.duration_since(start));

        if let Document::LaidOut { .. } = &self.next_document {
            self.document = self.next_document.take();

            if option_env!("WBE_TIMING_MODE").is_some() {
                std::process::exit(0);
            }
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
                            if let Document::None = self.next_document {
                                self.next_document = self.document.take().invalidate_layout();
                            } else {
                                self.next_document = self.next_document.take().invalidate_layout();
                            }
                            if let Err(e) = self.tick() {
                                error!("error: {}", e.to_string());
                                panic!();
                            }
                        }
                    }
                });
            });
    }
}
