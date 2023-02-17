use std::{
    fmt::Debug,
    sync::{Arc, RwLock, Weak},
};

use ab_glyph::ScaleFont;
use backtrace::Backtrace;
use egui::{vec2, FontFamily, Pos2, Rect};
use eyre::bail;
use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use tracing::{debug, trace};
use unicode_segmentation::UnicodeSegmentation;

use crate::{
    dom::{Node, NodeData, NodeType},
    font::FontInfo,
    paint::PaintText,
    parse::{html_word, HtmlWord},
    viewport::ViewportInfo,
    *,
};

const BLOCK: &[&str] = &[
    "html",
    "body",
    "article",
    "section",
    "nav",
    "aside",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hgroup",
    "header",
    "footer",
    "address",
    "p",
    "hr",
    "pre",
    "blockquote",
    "ol",
    "ul",
    "menu",
    "li",
    "dl",
    "dt",
    "dd",
    "figure",
    "figcaption",
    "main",
    "div",
    "table",
    "form",
    "fieldset",
    "legend",
    "details",
    "summary",
];

pub type LayoutRead<'n, T> = RwLockReadGuardRef<'n, OwnedLayout, T>;
pub type LayoutWrite<'n, T> = RwLockWriteGuardRefMut<'n, OwnedLayout, T>;

#[derive(Debug)]
pub struct OwnedLayout {
    pub node: Node,
    pub parent: Weak<RwLock<OwnedLayout>>,
    pub previous: Weak<RwLock<OwnedLayout>>,
    pub children: Vec<Layout>,
    pub mode: LayoutMode,
    pub display_list: Vec<PaintText>,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Document,
    Block,
    Inline,
}

#[derive(Debug)]
pub struct LayoutContext<'v> {
    viewport: &'v ViewportInfo,
    cursor: Pos2,
    max_ascent: f32,
    max_height: f32,
    line_display_list: Vec<PaintText>,
}

#[derive(Clone)]
pub struct Layout(Arc<RwLock<OwnedLayout>>);

impl Debug for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            format!("{:?}", self.read()).strip_prefix("Owned").unwrap()
        )
    }
}

impl Layout {
    fn with_mode(node: Node, mode: LayoutMode) -> Self {
        Self(Arc::new(RwLock::new(OwnedLayout {
            node,
            parent: Weak::new(),
            previous: Weak::new(),
            children: vec![],
            mode,
            display_list: vec![],
            rect: Rect::NAN,
        })))
    }

    fn mode_for(node: Node) -> Option<LayoutMode> {
        match node.r#type() {
            NodeType::Document => Some(LayoutMode::Block),
            NodeType::Element => {
                for child in &*node.children() {
                    for name in BLOCK {
                        if name.eq_ignore_ascii_case(&child.name()) {
                            return Some(LayoutMode::Block);
                        }
                    }
                }

                Some(if node.children().is_empty() {
                    LayoutMode::Block
                } else {
                    LayoutMode::Inline
                })
            }
            NodeType::Text => Some(LayoutMode::Block),
            NodeType::Comment => Some(LayoutMode::Block),
        }
    }

    pub fn document(node: Node) -> Self {
        assert!(matches!(&*node.data(), NodeData::Document));

        Self::with_mode(node, LayoutMode::Document)
    }

    pub fn block(node: Node) -> Self {
        Self::with_mode(node, LayoutMode::Block)
    }

    pub fn inline(node: Node) -> Self {
        Self::with_mode(node, LayoutMode::Inline)
    }

    pub fn append(&self, child: Layout) -> Self {
        child.write().parent = Arc::downgrade(&self.0);
        if let Some(last) = self.children().last() {
            child.write().previous = Arc::downgrade(&last.0);
        }
        self.write().children.push(child.clone());

        self.clone()
    }

    pub fn read(&self) -> LayoutRead<OwnedLayout> {
        if option_env!("WBE_DEBUG_RWLOCK").is_some() {
            dump_backtrace(Backtrace::new());
        }
        LayoutRead::new(self.0.try_read().unwrap())
    }

    pub fn write(&self) -> LayoutWrite<OwnedLayout> {
        if option_env!("WBE_DEBUG_RWLOCK").is_some() {
            dump_backtrace(Backtrace::new());
        }
        LayoutWrite::new(self.0.try_write().unwrap())
    }

    pub fn node(&self) -> LayoutRead<Node> {
        self.read().map(|x| &x.node)
    }

    pub fn mode(&self) -> LayoutMode {
        self.read().mode
    }

    pub fn children(&self) -> LayoutRead<[Layout]> {
        self.read().map(|x| &*x.children)
    }

    pub fn display_list(&self) -> LayoutRead<[PaintText]> {
        self.read().map(|x| &*x.display_list)
    }

    pub fn layout(&self, viewport: &ViewportInfo) -> eyre::Result<()> {
        // trace!(mode = ?self.mode(), node = %*self.node().data());

        let initial_rect = |previous: Option<&Layout>| {
            let mut result = self.read().rect;
            if let Some(previous) = previous {
                result.set_top(previous.read().rect.bottom());
            }
            result.set_height(0.0);
            result
        };

        match self.mode() {
            LayoutMode::Document => {
                self.write().rect = Rect::from_min_size(
                    viewport.rect.min + vec2(MARGIN, MARGIN),
                    vec2(viewport.rect.width() - 2.0 * MARGIN, 0.0),
                );

                let layout = Self::block(self.node().clone());
                layout.write().rect = initial_rect(None);
                layout.layout(viewport)?;
                self.write()
                    .display_list
                    .append(&mut layout.write().display_list);
                self.write().rect.max.y += layout.read().rect.height();
                self.append(layout);
                debug!(mode = ?self.mode(), height = self.read().rect.height(), display_list_len = self.read().display_list.len());
            }
            LayoutMode::Block => {
                // separate let releases RwLock read!
                let node = self.node().clone();
                match Self::mode_for(node) {
                    Some(LayoutMode::Block) => {
                        // temporary layout list releases RwLock read!
                        let mut layouts: Vec<Layout> = vec![];
                        for child in &*self.node().children() {
                            let layout = Self::block(child.clone());
                            layout.write().rect = initial_rect(layouts.last());
                            layout.layout(viewport)?;
                            layouts.push(layout);
                        }
                        for layout in layouts {
                            self.write()
                                .display_list
                                .append(&mut layout.write().display_list);
                            self.write().rect.max.y += layout.read().rect.height();
                            self.append(layout);
                        }
                    }
                    Some(LayoutMode::Inline) => {
                        let mut context = LayoutContext {
                            viewport,
                            cursor: self.read().rect.min,
                            max_ascent: 0.0,
                            max_height: 0.0,
                            line_display_list: vec![],
                        };

                        // separate let releases RwLock read!
                        let node = self.node().clone();
                        self.recurse(node, &mut context)?;
                        self.flush(&mut context)?;
                        self.write().rect.set_bottom(context.cursor.y);
                    }
                    _ => unreachable!(),
                }
            }
            LayoutMode::Inline => unreachable!(),
        }

        trace!(node = %*self.node().data(), outer = ?self.mode(), inner = ?Self::mode_for(self.node().clone()), height = self.read().rect.height());

        Ok(())
    }

    pub fn recurse(&self, node: Node, context: &mut LayoutContext) -> eyre::Result<()> {
        // trace!(mode = ?LayoutMode::Inline, node = %*node.data());
        if node.r#type() == NodeType::Text {
            self.text(node.clone(), context)?;
        } else {
            self.open_tag(&node.name());
            for child in &*node.children() {
                self.recurse(child.clone(), context)?;
            }
            self.close_tag(&node.name());
        }

        Ok(())
    }

    pub fn text(&self, node: Node, context: &mut LayoutContext) -> eyre::Result<()> {
        assert_eq!(node.r#type(), NodeType::Text);
        let font = FontInfo::new(
            FontFamily::Proportional,
            FONT_DATA,
            FONT_SIZE,
            context.viewport.scale,
        )?;
        let rect = self.read().rect;

        let mut input = &*node.value().unwrap();
        while !input.is_empty() {
            let (rest, token) = match html_word(input) {
                Ok(result) => result,
                // Err(nom::Err::Incomplete(_)) => ("", HtmlWord::Other(input)),
                Err(e) => bail!("{}; input={:?}", e, input),
            };
            let text = match token {
                HtmlWord::Space(_) => " ",
                HtmlWord::Other(x) => x,
            };
            for word in text.split_word_bounds() {
                let advance = word
                    .chars()
                    .map(|c| font.ab.h_advance(font.ab.glyph_id(c)))
                    .sum::<f32>()
                    / context.viewport.scale;
                let ascent = font.ab.ascent() / context.viewport.scale;
                let height = font.ab.height() / context.viewport.scale;
                if context.cursor.x + advance > rect.max.x {
                    // trace!(cursor = ?context.cursor, advance, max_x = rect.max.x);
                    self.flush(context)?;
                }
                context.max_ascent = context.max_ascent.max(ascent);
                context.max_height = context.max_height.max(height);
                let rect = Rect::from_min_size(context.cursor, vec2(advance, height));
                context
                    .line_display_list
                    .push(PaintText(rect, font.clone(), word.to_string()));
                context.cursor.x += advance;
            }
            input = rest;
        }
        // trace!(display_list_len = self.read().display_list.len());

        Ok(())
    }

    pub fn flush(&self, context: &mut LayoutContext) -> eyre::Result<()> {
        for mut paint in context.line_display_list.drain(..) {
            *paint.0.top_mut() += context.max_ascent - paint.1.ab.ascent() / context.viewport.scale;
            self.write().display_list.push(paint);
        }

        context.cursor.x = self.read().rect.min.x;
        context.cursor.y += context.max_height;
        context.max_ascent = 0.0;
        context.max_height = 0.0;

        Ok(())
    }

    pub fn open_tag(&self, _name: &str) {}

    pub fn close_tag(&self, _name: &str) {}
}
