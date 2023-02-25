use std::env::args;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use egui::{
    vec2, Align, Color32, Context, FontData, FontDefinitions, FontFamily, Frame, Rect, TextEdit,
};
use tracing::{error, instrument, trace, warn};

use wbe_browser::{Browser, Document, OwnedBrowser, OwnedDocument, RenderStatus};
use wbe_core::FONTS;
use wbe_layout::ViewportInfo;

fn main() -> eyre::Result<()> {
    // log to stdout (level configurable by RUST_LOG=debug)
    tracing_subscriber::fmt::init();

    let location = args()
        .nth(1)
        .unwrap_or("http://example.org/index.html".to_owned());

    let browser = Browser::wrap(OwnedBrowser {
        location,
        ..Default::default()
    });

    let (app, render_request_rx) = App::new(browser.clone());
    let renderer_thread = thread::spawn(move || loop {
        // wait for a request from the egui thread
        let Ok(mut request) = render_request_rx.recv() else { return };

        // discard all but the last pending request, to avoid wasting time
        // rendering against stale viewport geometry
        for next in render_request_rx.try_iter() {
            request = next;
        }

        if !request.viewport.is_valid() {
            warn!("renderer received render request, but viewport was invalid");
            continue;
        }

        let mut next_document = browser.read().next_document.write().take();
        if matches!(next_document, OwnedDocument::None) {
            warn!("renderer received render request, but there was no next_document");
            continue;
        }

        browser.set_status(RenderStatus::Load);
        request.egui_ctx.request_repaint();

        loop {
            next_document = match next_document {
                OwnedDocument::None => break,
                result @ OwnedDocument::Navigated { .. } => {
                    browser.set_status(RenderStatus::Load);
                    request.egui_ctx.request_repaint();
                    result
                }
                result @ OwnedDocument::Loaded { .. } => {
                    browser.set_status(RenderStatus::Parse);
                    request.egui_ctx.request_repaint();
                    result
                }
                result @ OwnedDocument::Parsed { .. } => {
                    browser.set_status(RenderStatus::Style);
                    request.egui_ctx.request_repaint();
                    result
                }
                result @ OwnedDocument::Styled { .. } => {
                    browser.set_status(RenderStatus::Layout);
                    request.egui_ctx.request_repaint();
                    result
                }
                result @ OwnedDocument::LaidOut { .. } => {
                    browser.write().document = Document::wrap(result);
                    if option_env!("WBE_TIMING_MODE").is_some() {
                        std::process::exit(0);
                    }
                    break;
                }
            };
            next_document = match next_document.tick(request.viewport.clone()) {
                Ok(result) => result,
                Err(e) => {
                    error!("error: {}", e.to_string());
                    break;
                }
            };
        }

        browser.set_status(RenderStatus::Done);
        request.egui_ctx.request_repaint();
    });

    let options = eframe::NativeOptions {
        initial_window_size: Some(vec2(1024.0, 768.0)),
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

            Box::new(app)
        }),
    )
    .unwrap();

    renderer_thread.join().unwrap();

    Ok(())
}

pub struct App {
    browser: Browser,
    render_request_tx: Sender<RenderRequest>,
}

pub struct RenderRequest {
    viewport: ViewportInfo,
    egui_ctx: Context,
}

impl App {
    fn new(browser: Browser) -> (Self, Receiver<RenderRequest>) {
        let (render_request_tx, render_request_rx) = channel();

        (
            Self {
                browser,
                render_request_tx,
            },
            render_request_rx,
        )
    }

    #[instrument(skip(self))]
    fn go(&mut self, egui_ctx: Context) {
        let location = self.browser.read().location.clone();
        self.browser.set_status(RenderStatus::Load);
        *self.browser.write().next_document.write() = OwnedDocument::Navigated { location };
        self.render_request_tx
            .send(RenderRequest {
                viewport: self.browser.read().viewport.clone(),
                egui_ctx,
            })
            .unwrap();
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::TopBottomPanel::top("location").show(ctx, |ui| {
            ui.allocate_ui_with_layout(
                ui.available_size(),
                egui::Layout::right_to_left(Align::Center),
                |ui| {
                    if ui.button("go").clicked() {
                        self.go(ctx.clone());
                    }
                    let status = self.browser.read().status;
                    if status != RenderStatus::Done {
                        ui.spinner();
                        ui.label(match status {
                            RenderStatus::Load => "load",
                            RenderStatus::Parse => "parse",
                            RenderStatus::Style => "style",
                            RenderStatus::Layout => "layout",
                            RenderStatus::Done => unreachable!(),
                        });
                    }
                    ui.add_sized(
                        ui.available_size(),
                        TextEdit::singleline(&mut *self.browser.location_mut()),
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
                            self.browser.write().scroll = outer_position - inner_position;

                            // e.g. [788 564]
                            let client_size = ui.available_size();

                            // e.g. [[0 24] - [788 800]]
                            Rect::from_min_size(outer_position, client_size)
                        };

                        // e.g. [788 564]
                        let mut scroll_size = viewport_rect.size();

                        let document = self.browser.read().document.clone();
                        let document = document.write();
                        let mut browser = self.browser.write();
                        let new_viewport = browser
                            .viewport
                            .update(viewport_rect, ctx.pixels_per_point())
                            .clone();
                        if let OwnedDocument::LaidOut {
                            layout, viewport, ..
                        } = &*document
                        {
                            // expand scroll_rect where needed to fit page contents
                            scroll_size.x = scroll_size.x.max(layout.read().rect.width());
                            scroll_size.y = scroll_size.y.max(layout.read().rect.height());

                            // paint the layout tree translated by -self.scroll (since we do the
                            // translate ourselves and not ScrollArea, it’s not cheating)
                            OwnedDocument::paint(ui, layout, viewport, browser.scroll);

                            if *viewport != new_viewport {
                                let has_next_document =
                                    !matches!(*browser.next_document.read(), OwnedDocument::None);
                                if has_next_document {
                                    let next_document =
                                        browser.next_document.write().take().invalidate_layout();
                                    browser.next_document = Document::wrap(next_document);
                                } else {
                                    let next_document = document.invalidate_layout();
                                    browser.next_document = Document::wrap(next_document);
                                }
                                self.render_request_tx
                                    .send(RenderRequest {
                                        viewport: browser.viewport.clone(),
                                        egui_ctx: ctx.clone(),
                                    })
                                    .unwrap();
                            }
                        }

                        let layout_rect = match &*document {
                            OwnedDocument::LaidOut { layout, .. } => layout.read().rect,
                            _ => Rect::NAN,
                        };
                        trace!(
                            ?outer_rect, inner_rect = ?ui.cursor(),
                            ?layout_rect, ?viewport_rect,
                            ?scroll_size, scroll = ?self.browser.read().scroll,
                        );

                        // set range of scrollbars
                        ui.set_min_size(scroll_size);
                    });
            });

        // now that we have a valid viewport, go if needed
        assert!(self.browser.read().viewport.is_valid());
        let first_update = self.browser.read().first_update;
        if first_update {
            self.browser.write().first_update = false;
            if !self.browser.read().location.is_empty() {
                self.go(ctx.clone());
            }
        }
    }
}
