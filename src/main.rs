use std::time::Instant;
use std::{env::args, str};

use egui::{
    vec2, Align, Align2, Color32, FontData, FontDefinitions, FontFamily, Frame, Rect, TextEdit, Ui,
    Vec2,
};
use eyre::bail;
use tracing::{debug, error, info, instrument, trace};

use wbe::document::Document;
use wbe::dom::{Node, NodeData};
use wbe::layout::Layout;
use wbe::parse::{html_token, HtmlToken};
use wbe::viewport::ViewportInfo;
use wbe::*;

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
            for &(name, data) in FONTS {
                font_definitions
                    .font_data
                    .insert(name.to_owned(), FontData::from_static(data));
                font_definitions
                    .families
                    .insert(FontFamily::Name(name.into()), vec![name.to_owned()]);
            }
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
                // Err(nom::Err::Incomplete(_)) => ("", HtmlToken::Text(input)),
                Err(e) => bail!("{}; input={:?}", e, input),
            };
            match token {
                HtmlToken::Comment(text) => {
                    parent.append(&[Node::comment(text.to_owned())]);
                }
                HtmlToken::Script(_attrs, text) => {
                    // TODO attrs
                    parent.append(&[Node::element("script".to_owned(), vec![])
                        .append(&[Node::text(text.to_owned())])]);
                }
                HtmlToken::Style(_attrs, text) => {
                    // TODO attrs
                    parent.append(&[Node::element("style".to_owned(), vec![])
                        .append(&[Node::text(text.to_owned())])]);
                }
                HtmlToken::Tag(true, name, _attrs) => {
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
                HtmlToken::Tag(false, name, _attrs) => {
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
        let layout = Layout::document(dom.clone());
        layout.layout(&viewport)?;

        Ok(Document::LaidOut {
            location,
            response_body,
            dom,
            layout,
            viewport,
        })
    }

    #[instrument(skip(self, ui, layout))]
    fn paint(&self, ui: &Ui, layout: &Layout, viewport: &ViewportInfo) {
        let painter = ui.painter();
        for paint in &*layout.display_list() {
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
                egui::Layout::right_to_left(Align::Center),
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
                // e.g. [[0 24] - [800 inf]] → [0 24]
                let outer_rect = ui.cursor();
                let outer_position = outer_rect.min;

                // the egui ScrollArea provides interactive scrollbars, plus the ability to read
                // scroll wheel input via ui.input(|i|i.scroll_delta) for relative, or for absolute,
                // ui.cursor().min minus the outer ui.cursor().min. in fact, i can’t find any way
                // at all to read scroll wheel input without a ScrollArea!
                egui::ScrollArea::both()
                    .always_show_scroll(true)
                    .auto_shrink([false, false])
                    .min_scrolled_width(0.0)
                    .min_scrolled_height(0.0)
                    .show(ui, |ui| {
                        let viewport_rect = {
                            // e.g. [0 -26] when scrolled by [0 50]
                            let inner_position = ui.cursor().min;

                            // e.g. [0 50]
                            self.scroll = outer_position - inner_position;

                            // e.g. [788 564]
                            let client_size = ui.available_size();

                            // e.g. [[0 24] - [788 800]]
                            Rect::from_min_size(outer_position, client_size)
                        };

                        // e.g. [788 564]
                        let mut scroll_size = viewport_rect.size();

                        if let Document::LaidOut {
                            layout, viewport, ..
                        } = &self.document
                        {
                            // expand scroll_rect where needed to fit page contents
                            scroll_size.x = scroll_size.x.max(layout.read().rect.width());
                            scroll_size.y = scroll_size.y.max(layout.read().rect.height());

                            // paint the layout tree translated by -self.scroll (since we do the
                            // translate ourselves and not ScrollArea, it’s not cheating)
                            self.paint(ui, layout, viewport);

                            if viewport
                                != self.viewport.update(viewport_rect, ctx.pixels_per_point())
                            {
                                if let Document::None = self.next_document {
                                    self.next_document = self.document.take().invalidate_layout();
                                } else {
                                    self.next_document =
                                        self.next_document.take().invalidate_layout();
                                }
                                if let Err(e) = self.tick() {
                                    error!("error: {}", e.to_string());
                                    panic!();
                                }
                            }
                        }

                        let layout_rect = match &self.document {
                            Document::LaidOut { layout, .. } => layout.read().rect,
                            _ => Rect::NAN,
                        };
                        trace!(
                            ?outer_rect, inner_rect = ?ui.cursor(),
                            ?layout_rect, ?viewport_rect,
                            ?scroll_size, scroll = ?self.scroll,
                        );

                        // set range of scrollbars
                        ui.set_min_size(scroll_size);
                    });
            });
    }
}
