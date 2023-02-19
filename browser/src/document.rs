use std::mem::{size_of, size_of_val};
use std::sync::{Arc, RwLock};
use std::time::Instant;
use std::{fmt::Debug, mem::swap, str};

use backtrace::Backtrace;
use egui::{Align2, Color32, Ui, Vec2};
use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use tracing::{debug, error, info, instrument, trace, warn};

use wbe_core::dump_backtrace;
use wbe_css_parser::{css_file, css_ident};
use wbe_dom::{Node, NodeData, OwnedNode};
use wbe_html_parser::parse_html;
use wbe_layout::Paint;
use wbe_layout::{viewport::ViewportInfo, Layout, OwnedLayout};

#[derive(Default, Clone)]
pub struct Document(Arc<RwLock<OwnedDocument>>);

pub type DocumentRead<'n, T> = RwLockReadGuardRef<'n, OwnedDocument, T>;
pub type DocumentWrite<'n, T> = RwLockWriteGuardRefMut<'n, OwnedDocument, T>;

impl Document {
    pub fn wrap(inner: OwnedDocument) -> Self {
        Self(Arc::new(RwLock::new(inner)))
    }

    pub fn read(&self) -> DocumentRead<OwnedDocument> {
        if option_env!("WBE_DEBUG_RWLOCK").is_some() {
            dump_backtrace(Backtrace::new());
        }
        DocumentRead::new(self.0.read().unwrap())
    }

    pub fn write(&self) -> DocumentWrite<OwnedDocument> {
        if option_env!("WBE_DEBUG_RWLOCK").is_some() {
            dump_backtrace(Backtrace::new());
        }
        DocumentWrite::new(self.0.write().unwrap())
    }
}

#[derive(Debug, Default, Clone)]
pub enum OwnedDocument {
    #[default]
    None,
    Navigated {
        location: String,
    },
    Loaded {
        location: String,
        response_body: String,
    },
    Parsed {
        location: String,
        response_body: String,
        dom: Node,
    },
    LaidOut {
        location: String,
        response_body: String,
        dom: Node,
        layout: Layout,
        viewport: ViewportInfo,
    },
}

impl OwnedDocument {
    pub fn take(&mut self) -> Self {
        let mut result = Self::None;
        swap(self, &mut result);

        result
    }

    pub fn invalidate_layout(&self) -> Self {
        match self.clone() {
            OwnedDocument::LaidOut {
                location,
                response_body,
                dom,
                ..
            } => OwnedDocument::Parsed {
                location,
                response_body,
                dom,
            },
            other => other,
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            OwnedDocument::None => "None",
            OwnedDocument::Navigated { .. } => "Navigated",
            OwnedDocument::Loaded { .. } => "Loaded",
            OwnedDocument::Parsed { .. } => "Parsed",
            OwnedDocument::LaidOut { .. } => "LaidOut",
        }
    }

    pub fn size(&self) -> Vec2 {
        let mut result = Vec2::ZERO;
        if let Self::LaidOut { layout, .. } = self {
            for paint in &*layout.display_list() {
                result = result.max(paint.rect().max.to_vec2());
            }
        }

        result
    }

    pub fn scroll_limit(&self) -> Vec2 {
        let mut result = self.size();
        if let Self::LaidOut { viewport, .. } = self {
            result -= viewport.rect.size();
        }

        result.max(Vec2::ZERO)
    }

    #[instrument]
    fn load(location: String) -> eyre::Result<OwnedDocument> {
        let (_headers, body) = wbe_http::request(&location)?;

        Ok(OwnedDocument::Loaded {
            location,
            // TODO: hard-coding utf-8 is not correct in practice
            response_body: str::from_utf8(&body)?.to_owned(),
        })
    }

    #[instrument(skip(response_body))]
    fn parse(location: String, response_body: String) -> eyre::Result<OwnedDocument> {
        let dom = parse_html(&response_body)?;
        debug!(%dom);

        for node in dom.descendants().filter(|x| &*x.name() == "style") {
            let text = node.text_content();
            let rules = match css_file(&text) {
                Ok(("", result)) => result,
                Ok((rest, result)) => {
                    warn!("trailing text in css file: {:?}", rest);
                    result
                }
                Err(error) => {
                    error!(?error);
                    continue;
                }
            };

            for (selectors, declarations) in rules {
                for (selector, combinators) in selectors {
                    if !combinators.is_empty() {
                        continue; // TODO
                    }
                    if selector.len() != 1 {
                        continue; // TODO
                    }
                    let selector = selector[0];
                    if css_ident(selector).is_err() {
                        continue; // TODO
                    }
                    for node in dom
                        .descendants()
                        .filter(|x| x.name().eq_ignore_ascii_case(selector))
                    {
                        trace!(selector, node = %*node.data());
                        let mut style = node.data().style();
                        for &(name, value) in &declarations {
                            match name {
                                "background-color" => {
                                    style.background_color = Some(value.to_owned())
                                }
                                "color" => style.color = Some(value.to_owned()),
                                _ => {}
                            }
                        }
                        node.data_mut().set_style(style);
                    }
                }
            }
        }

        Ok(OwnedDocument::Parsed {
            location,
            response_body,
            dom,
        })
    }

    #[instrument(skip(response_body, dom))]
    fn layout(
        viewport: ViewportInfo,
        location: String,
        response_body: String,
        dom: Node,
    ) -> eyre::Result<OwnedDocument> {
        let layout = Layout::document(dom.clone());
        layout.layout(&viewport)?;

        Ok(OwnedDocument::LaidOut {
            location,
            response_body,
            dom,
            layout,
            viewport,
        })
    }

    #[instrument(skip(ui, layout))]
    pub fn paint(ui: &Ui, layout: &Layout, viewport: &ViewportInfo, scroll: Vec2) {
        let painter = ui.painter();
        for paint in &*layout.display_list() {
            let rect = paint.rect().translate(-scroll);
            if rect.intersects(viewport.rect) {
                match paint {
                    Paint::Text(_, color, font, text) => {
                        // painter.rect_stroke(rect, 0.0, Stroke::new(1.0, Color32::from_rgb(255, 0, 255)));
                        painter.text(rect.min, Align2::LEFT_TOP, text, font.egui.clone(), *color);
                    }
                    Paint::Fill(_, color) => {
                        painter.rect(rect, 0.0, *color, (0.0, Color32::TRANSPARENT));
                    }
                }
            }
        }
    }

    #[instrument(skip(self, viewport))]
    pub fn tick(self, viewport: ViewportInfo) -> eyre::Result<OwnedDocument> {
        let start = Instant::now();
        let result = match self {
            OwnedDocument::None => return Ok(self),
            OwnedDocument::Navigated { location } => Self::load(location)?,
            OwnedDocument::Loaded {
                location,
                response_body,
            } => Self::parse(location, response_body)?,
            OwnedDocument::Parsed {
                location,
                response_body,
                dom,
            } => Self::layout(viewport, location, response_body, dom)?,
            document @ OwnedDocument::LaidOut { .. } => document,
        };

        let now = Instant::now();
        info!(status = result.status(), duration = ?now.duration_since(start), memory_usage = result.memory_usage());

        Ok(result)
    }

    #[instrument(skip(self))]
    pub fn memory_usage(&self) -> usize {
        fn size_of_string(x: &String) -> usize {
            // x (vec (ptr + len + capacity)) + data
            size_of_val(x) + x.capacity()
        }
        fn size_of_vec<T>(x: &Vec<T>) -> usize {
            // x (ptr + len + capacity) + data
            size_of_val(x) + x.capacity() * size_of::<T>()
        }
        fn size_of_dom_tree(x: &Node) -> usize {
            // x (arc (ptr)) + strong + weak + owned
            size_of_val(x) + 2 * size_of::<usize>() + size_of_owned_node(&x.read())
        }
        fn size_of_owned_node(x: &OwnedNode) -> usize {
            // x (parent (weak (ptr)) + children direct + inner direct) - inner direct + inner total + children indirect
            size_of_val(x) - size_of_val(&x.inner)
                + size_of_node_data(&x.inner)
                + x.children
                    .iter()
                    .map(|x| size_of_dom_tree(x))
                    .sum::<usize>()
        }
        fn size_of_node_data(x: &NodeData) -> usize {
            // x (enum (discriminant + string direct + vec direct)) - direct + fields
            size_of_val(x)
                - match x {
                    NodeData::Document => 0,
                    NodeData::Element(n, a, _) => size_of_val(n) + size_of_val(a),
                    NodeData::Text(t) => size_of_val(t),
                    NodeData::Comment(t) => size_of_val(t),
                }
                + match x {
                    NodeData::Document => 0,
                    NodeData::Element(n, a, _) => size_of_string(n) + size_of_vec(a),
                    NodeData::Text(t) => size_of_string(t),
                    NodeData::Comment(t) => size_of_string(t),
                }
        }
        fn size_of_layout_tree(x: &Layout) -> usize {
            // x (arc (ptr)) + strong + weak + owned
            size_of_val(x) + 2 * size_of::<usize>() + size_of_owned_layout(&x.read())
        }
        fn size_of_owned_layout(x: &OwnedLayout) -> usize {
            // x (rest + children direct + display_list direct) - display_list indirect + display_list total + children indirect
            size_of_val(x) - size_of_val(&x.display_list)
                + size_of_vec(&x.display_list)
                + x.children
                    .iter()
                    .map(|x| size_of_layout_tree(x))
                    .sum::<usize>()
        }

        match self {
            Self::None => size_of_val(self),
            Self::Navigated { location } => size_of_val(&Self::None) + size_of_string(location),
            Self::Loaded {
                location,
                response_body,
            } => {
                size_of_val(&Self::None) + size_of_string(location) + size_of_string(response_body)
            }
            Self::Parsed {
                location,
                response_body,
                dom,
            } => {
                size_of_val(&Self::None)
                    + size_of_string(location)
                    + size_of_string(response_body)
                    + size_of_dom_tree(dom)
            }
            Self::LaidOut {
                location,
                response_body,
                dom,
                layout,
                viewport: _,
            } => {
                debug!(
                    dom_tree_size = size_of_dom_tree(dom),
                    layout_tree_size = size_of_layout_tree(layout)
                );
                size_of_val(&Self::None)
                    + size_of_string(location)
                    + size_of_string(response_body)
                    + size_of_dom_tree(dom)
                    + size_of_layout_tree(layout)
            }
        }
    }
}
