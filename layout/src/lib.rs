#![feature(array_chunks)]
#![feature(iter_array_chunks)]
#![feature(stmt_expr_attributes)]

pub mod font;
pub mod paint;
pub mod viewport;

pub use crate::{font::FontInfo, paint::Paint, viewport::ViewportInfo};

use std::{
    fmt::Debug,
    sync::{Arc, RwLock, Weak},
};

use ab_glyph::ScaleFont;
use backtrace::Backtrace;
use egui::{vec2, Color32, FontFamily, Pos2, Rect};
use eyre::bail;
use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use tracing::{debug, instrument, trace, warn};
use unicode_segmentation::UnicodeSegmentation;

use wbe_core::{dump_backtrace, FONTS, FONT_SIZE, MARGIN};
use wbe_dom::{
    style::{CssDisplay, CssFontStyle, CssFontWeight},
    Node, NodeData, NodeType,
};
use wbe_html_lexer::{html_word, HtmlWord};

const DISPLAY_NONE: &[&str] = &["#comment", "head", "title", "script", "style"];
const DISPLAY_BLOCK: &[&str] = &[
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
    pub node: Option<Node>,
    pub inlines: Vec<Node>,
    pub parent: Weak<RwLock<OwnedLayout>>,
    pub previous: Weak<RwLock<OwnedLayout>>,
    pub children: Vec<Layout>,
    pub display_list: Vec<Paint>,
    pub rect: Rect,
}

struct DocumentContext<'v, 'p> {
    viewport: &'v ViewportInfo,
    display_list: &'p mut Vec<Paint>,
    block: BlockContext,
}

#[derive(Debug, Clone)]
struct BlockContext {}

#[derive(Debug)]
struct InlineContext {
    cursor: Pos2,
    max_ascent: f32,
    max_height: f32,
    line_display_list: Vec<Paint>,
}

#[derive(Clone)]
pub struct Layout(Arc<RwLock<OwnedLayout>>);

impl Debug for Layout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut tuple = f.debug_tuple("\x1B[1;95mL{\x1B[0m");
        if let Some(node) = self.node() {
            tuple.field(&format_args!("{}", &*node.data()));
        }
        for layout in &*self.children() {
            tuple.field(&format_args!("b={:?}", layout));
        }
        for node in &*self.inlines() {
            tuple.field(&format_args!("i={}", &*node.data()));
        }

        tuple.finish()?;
        f.write_str("\x1B[1;95m}\x1B[0m")
    }
}

impl Layout {
    pub fn anonymous(nodes: impl IntoIterator<Item = Node>) -> Self {
        Self(Arc::new(RwLock::new(OwnedLayout {
            node: None,
            inlines: nodes.into_iter().collect(),
            parent: Weak::new(),
            previous: Weak::new(),
            children: vec![],
            display_list: vec![],
            rect: Rect::NAN,
        })))
    }

    pub fn with_node(node: Node) -> Self {
        Self(Arc::new(RwLock::new(OwnedLayout {
            node: Some(node),
            inlines: vec![],
            parent: Weak::new(),
            previous: Weak::new(),
            children: vec![],
            display_list: vec![],
            rect: Rect::NAN,
        })))
    }

    /// returns true iff the given subtree can be skipped entirely.
    fn is_skippable(node: &Node) -> bool {
        match node.data().style().display() {
            CssDisplay::None => true,
            _ => false,
        }
    }

    /// returns true iff the given Node forces boxes to be created.
    fn is_block_level(node: &Node) -> bool {
        // no if text or comment
        match node.r#type() {
            NodeType::Text => return false,
            NodeType::Comment => return false,
            _ => {}
        }

        // yes if we have a block-level ‘display’
        let blocky_display = match node.data().style().display() {
            CssDisplay::None => false,
            CssDisplay::Inline => false,
            CssDisplay::Block => true,
            CssDisplay::InlineBlock => true,
            CssDisplay::ListItem => true,
        };

        // or if any of our children force boxes to be created
        blocky_display || node.children().iter().any(|x| Self::is_block_level(x))
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

    pub fn node(&self) -> Option<LayoutRead<Node>> {
        self.read()
            .try_map(|x| match &x.node {
                Some(x) => Ok(x),
                None => Err(()),
            })
            .ok()
    }

    pub fn inlines(&self) -> LayoutRead<[Node]> {
        self.read().map(|x| &*x.inlines)
    }

    pub fn children(&self) -> LayoutRead<[Layout]> {
        self.read().map(|x| &*x.children)
    }

    pub fn display_list(&self) -> LayoutRead<[Paint]> {
        self.read().map(|x| &*x.display_list)
    }

    pub fn layout(&self, viewport: &ViewportInfo) -> eyre::Result<()> {
        assert_eq!(self.inlines().len(), 0);
        assert_eq!(self.node().unwrap().r#type(), NodeType::Document);

        let mut display_list = vec![];
        let mut dc = DocumentContext {
            viewport,
            display_list: &mut display_list,
            block: BlockContext {},
        };

        self.write().rect =
            Rect::from_min_size(dc.viewport.rect.min, vec2(dc.viewport.rect.width(), 0.0));
        self.f(&mut dc)?;
        self.write().display_list = display_list;

        Ok(())
    }

    #[instrument(skip(dc))]
    fn f(&self, dc: &mut DocumentContext) -> eyre::Result<()> {
        // trace!(mode = ?self.mode(), node = %*self.node().data());

        // save where we started, for background paint
        let i = dc.display_list.len();

        // boxes inside this box
        let mut boxes = vec![];

        // nodes that will go in the next such box
        let mut inlines = vec![];

        // for all children, accumulate inlines in
        // the nodes vec. when we see a block-level descendant, make an
        // anonymous box for those inlines, then make a box for that
        // block-level descendant, then go back to the accumulate step.
        let candidates: Vec<Node> = self
            .node()
            .iter()
            .flat_map(|n| n.children().to_owned())
            .collect();
        for child in candidates {
            if Self::is_block_level(&child) {
                if !inlines.is_empty() {
                    let layout = Self::anonymous(inlines.drain(..));
                    debug!(box_child = %*child.data(), before = ?layout);
                    boxes.push(layout);
                } else {
                    debug!(box_child = %*child.data());
                }
                let layout = Self::with_node(child);
                boxes.push(layout);
            } else if !Self::is_skippable(&child) {
                debug!(line_child = %*child.data());
                inlines.push(child);
            }
        }

        // if there are any inlines left over:
        if !inlines.is_empty() {
            // if there are other boxes inside this box, create an
            // anonymous box for them.
            if !boxes.is_empty() {
                let layout = Self::anonymous(inlines.drain(..));
                boxes.push(layout);
            }
            // otherwise, there were only inlines, so make all of them
            // our own nodes.
            else {
                self.write().inlines.append(&mut inlines);
            }
        }

        #[rustfmt::skip]
        assert!(
            // either we have only boxes inside (anonymous or otherwise)
            (!boxes.is_empty() && self.inlines().is_empty())
            ||
            // or we have inlines inside with no boxes inside
            (!self.inlines().is_empty() && boxes.is_empty())
            ||
            // or we have neither if(?) the document is entirely empty
            (boxes.is_empty() && self.inlines().is_empty())
            ,
            "bad layout! {:?}", self
        );

        if !boxes.is_empty() {
            for layout in boxes {
                let node = layout.node().map(|x| x.clone());
                self.append(layout.clone());
                let previous = layout.read().previous.upgrade().map(Self);
                layout.write().rect = self.read().rect;
                if let Some(previous) = previous {
                    layout.write().rect.set_top(previous.read().rect.bottom());
                }
                if let Some(node) = node {
                    let available = layout.read().rect.width();
                    let font_size = node.data().style().font_size();
                    debug!(width = ?node.data().style().width, available, font_size, node = %*node.data());
                    layout
                        .write()
                        .rect
                        .set_width(dbg!(node.data().style().width(available, font_size)));
                }
                layout.write().rect.set_height(0.0);
                layout.f(dc)?;
                self.write().rect.set_bottom(layout.read().rect.bottom());
                trace!(rect = ?self.read().rect, extender = ?layout.read().rect);
            }
        } else if !self.inlines().is_empty() {
            let mut ic = InlineContext {
                cursor: self.read().rect.min,
                max_ascent: 0.0,
                max_height: 0.0,
                line_display_list: vec![],
            };

            // separate let releases RwLock read!
            let nodes = self.inlines().to_owned();
            for node in nodes {
                self.recurse(node.clone(), dc, &mut ic)?;
                self.flush(dc, &mut ic)?;
                self.write().rect.set_bottom(ic.cursor.y);
            }
        }

        if let Some(node) = self.node() {
            dc.display_list.insert(
                i,
                Paint::Fill(self.read().rect, node.data().style().background_color()),
            );
        }

        Ok(())
    }

    fn recurse(
        &self,
        node: Node,
        dc: &mut DocumentContext,
        ic: &mut InlineContext,
    ) -> eyre::Result<()> {
        // trace!(mode = ?LayoutMode::Inline, node = %*node.data());
        match node.r#type() {
            NodeType::Document => unreachable!(),
            NodeType::Element => {
                for child in &*node.children() {
                    self.recurse(child.clone(), dc, ic)?;
                }
            }
            NodeType::Text => {
                self.text(node.clone(), dc, ic)?;
            }
            NodeType::Comment => return Ok(()),
        }

        Ok(())
    }

    fn text(
        &self,
        node: Node,
        dc: &mut DocumentContext,
        ic: &mut InlineContext,
    ) -> eyre::Result<()> {
        assert_eq!(node.r#type(), NodeType::Text);
        let style = node.data().style();
        let font = FontInfo::new(
            FontFamily::Name(match (style.font_weight(), style.font_style()) {
                (CssFontWeight::Normal, CssFontStyle::Normal) => FONTS[0].0.into(),
                (CssFontWeight::Bold, CssFontStyle::Normal) => FONTS[1].0.into(),
                (CssFontWeight::Normal, CssFontStyle::Italic) => FONTS[2].0.into(),
                (CssFontWeight::Bold, CssFontStyle::Italic) => FONTS[3].0.into(),
            }),
            match (style.font_weight(), style.font_style()) {
                (CssFontWeight::Normal, CssFontStyle::Normal) => FONTS[0].1,
                (CssFontWeight::Bold, CssFontStyle::Normal) => FONTS[1].1,
                (CssFontWeight::Normal, CssFontStyle::Italic) => FONTS[2].1,
                (CssFontWeight::Bold, CssFontStyle::Italic) => FONTS[3].1,
            },
            style.font_size(),
            dc.viewport.scale,
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
                    / dc.viewport.scale;
                let ascent = font.ab.ascent() / dc.viewport.scale;
                let height = font.ab.height() / dc.viewport.scale;
                if ic.cursor.x + advance > rect.max.x {
                    // trace!(cursor = ?context.cursor, advance, max_x = rect.max.x);
                    self.flush(dc, ic)?;
                }
                ic.max_ascent = ic.max_ascent.max(ascent);
                ic.max_height = ic.max_height.max(height);
                let rect = Rect::from_min_size(ic.cursor, vec2(advance, height));
                ic.line_display_list.push(Paint::Text(
                    rect,
                    style.color(),
                    font.clone(),
                    word.to_string(),
                ));
                ic.cursor.x += advance;
            }
            input = rest;
        }
        // trace!(display_list_len = self.read().display_list.len());

        Ok(())
    }

    fn flush(&self, dc: &mut DocumentContext, ic: &mut InlineContext) -> eyre::Result<()> {
        for mut text in ic.line_display_list.drain(..) {
            match &mut text {
                Paint::Text(rect, _, font, _) => {
                    *rect = rect.translate(vec2(
                        0.0,
                        ic.max_ascent - font.ab.ascent() / dc.viewport.scale,
                    ));
                }
                _ => unreachable!(),
            }

            dc.display_list.push(text);
        }

        ic.cursor.x = self.read().rect.min.x;
        ic.cursor.y += ic.max_height;
        ic.max_ascent = 0.0;
        ic.max_height = 0.0;

        Ok(())
    }
}
