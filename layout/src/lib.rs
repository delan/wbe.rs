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
use egui::{vec2, FontFamily, Pos2, Rect};
use eyre::bail;
use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use tracing::{debug, instrument, trace, warn};
use unicode_segmentation::UnicodeSegmentation;

use wbe_core::{dump_backtrace, FONTS};
use wbe_dom::{
    style::{CssDisplay, CssFontStyle, CssFontWeight, CssQuad, CssTextAlign},
    Node, NodeType, Style,
};
use wbe_html_lexer::{html_word, HtmlWord};

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

    margin: CssQuad<f32>,
    border: CssQuad<f32>,
    padding: CssQuad<f32>,
    text_align: CssTextAlign,
}

struct DocumentContext<'v, 'p> {
    viewport: &'v ViewportInfo,
    display_list: &'p mut Vec<Paint>,
}

#[derive(Debug)]
struct InlineContext {
    content_rect: Rect,
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
    pub fn anonymous(&self, nodes: impl IntoIterator<Item = Node>) -> Self {
        Self(Arc::new(RwLock::new(OwnedLayout {
            node: None,
            inlines: nodes.into_iter().collect(),
            parent: Weak::new(),
            previous: Weak::new(),
            children: vec![],
            display_list: vec![],
            rect: Rect::NAN,

            margin: CssQuad::one(0.0),
            border: CssQuad::one(0.0),
            padding: CssQuad::one(0.0),
            text_align: self.read().text_align,
        })))
    }

    pub fn with_node(node: Node, available: f32) -> Self {
        let style = node.data().style();
        let font_size = style.font_size();

        Self(Arc::new(RwLock::new(OwnedLayout {
            node: Some(node),
            inlines: vec![],
            parent: Weak::new(),
            previous: Weak::new(),
            children: vec![],
            display_list: vec![],
            rect: Rect::NAN,

            margin: style.margin().map_or(Style::initial().margin(), |x| {
                Some(x.resolve(available, font_size))
            }),
            border: style
                .border_width()
                .map_or(&Style::initial().border_width(), |x| {
                    x.resolve_no_percent(font_size)
                }),
            padding: style.padding().map_or(Style::initial().padding(), |x| {
                Some(x.resolve(available, font_size))
            }),
            text_align: style.text_align(),
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

    #[instrument(skip(viewport))]
    pub fn layout(&self, viewport: &ViewportInfo) -> eyre::Result<()> {
        assert_eq!(self.inlines().len(), 0);
        assert_eq!(self.node().unwrap().r#type(), NodeType::Document);

        let mut display_list = vec![];
        let mut dc = DocumentContext {
            viewport,
            display_list: &mut display_list,
        };

        self.write().rect =
            Rect::from_min_size(dc.viewport.rect.min, vec2(dc.viewport.rect.width(), 0.0));
        self.f(&mut dc)?;
        self.write().display_list = display_list;

        Ok(())
    }

    fn f(&self, dc: &mut DocumentContext) -> eyre::Result<()> {
        // trace!(mode = ?self.mode(), node = %*self.node().data());

        // save where we started, for background paint
        let i = dc.display_list.len();

        let (mut margin_rect, mut padding_rect, mut border_rect, mut content_rect) =
            if self.node().is_some() {
                let mut rect = self.read().rect;
                let margin_rect = rect;
                rect.set_top(rect.top() + self.read().margin.top_unwrap());
                rect.set_left(rect.left() + self.read().margin.left_unwrap());
                rect.set_right(rect.right() - self.read().margin.right_unwrap());
                let border_rect = rect;
                rect.set_top(rect.top() + self.read().border.top_unwrap());
                rect.set_left(rect.left() + self.read().border.left_unwrap());
                rect.set_right(rect.right() - self.read().border.right_unwrap());
                let padding_rect = rect;
                rect.set_top(rect.top() + self.read().padding.top_unwrap());
                rect.set_left(rect.left() + self.read().padding.left_unwrap());
                rect.set_right(rect.right() - self.read().padding.right_unwrap());
                let content_rect = rect;

                (margin_rect, padding_rect, border_rect, content_rect)
            } else {
                let rect = self.read().rect;

                (rect, rect, rect, rect)
            };

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
                    let layout = self.anonymous(inlines.drain(..));
                    debug!(box_child = %*child.data(), before = ?layout);
                    boxes.push(layout);
                } else {
                    debug!(box_child = %*child.data());
                }
                let layout = Self::with_node(child, self.read().rect.width());
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
                let layout = self.anonymous(inlines.drain(..));
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
                layout.write().rect = content_rect;
                if let Some(previous) = previous {
                    layout.write().rect.set_top(previous.read().rect.bottom());
                }
                if let Some(node) = node {
                    let available = layout.read().rect.width();
                    let margin_left = *layout.read().margin.left_unwrap();
                    let border_left = *layout.read().border.left_unwrap();
                    let padding_left = *layout.read().padding.left_unwrap();
                    let padding_right = *layout.read().padding.right_unwrap();
                    let border_right = *layout.read().border.right_unwrap();
                    let margin_right = *layout.read().margin.right_unwrap();
                    trace!(
                        node = %*node.data(),
                        width = ?node.data().style().width,
                        available,
                        mbppbm = ?(margin_left, border_left, padding_left, padding_right, border_right, margin_right),
                    );
                    layout
                        .write()
                        .rect
                        .set_width(node.data().style().box_width(available));
                }
                layout.write().rect.set_height(0.0);
                layout.f(dc)?;
                content_rect.set_bottom(layout.read().rect.bottom());
                trace!(rect = ?self.read().rect, extender = ?layout.read().rect);
            }
        } else if !self.inlines().is_empty() {
            let mut ic = InlineContext {
                content_rect,
                cursor: content_rect.min,
                max_ascent: 0.0,
                max_height: 0.0,
                line_display_list: vec![],
            };

            // separate let releases RwLock read!
            let nodes = self.inlines().to_owned();
            for node in nodes {
                self.recurse(node.clone(), dc, &mut ic)?;
            }
            self.flush(dc, &mut ic)?;
            content_rect.set_bottom(ic.cursor.y);
        }

        if let Some(node) = self.node() {
            if let Some(height) = node.data().style().box_height() {
                trace!(node = %*node.data(), height);
                content_rect.set_bottom(content_rect.top() + height);
            }
        }
        padding_rect.set_bottom(content_rect.bottom() + self.read().padding.bottom_unwrap());
        border_rect.set_bottom(padding_rect.bottom() + self.read().border.bottom_unwrap());
        margin_rect.set_bottom(border_rect.bottom() + self.read().margin.bottom_unwrap());
        self.write().rect.set_bottom(margin_rect.bottom());

        if let Some(node) = self.node() {
            let style = node.data().style();
            let current_color = style.color();
            dc.display_list.insert(
                i,
                Paint::Fill(
                    padding_rect,
                    style.background_color().resolve(current_color),
                ),
            );

            let border_top_rect = Rect::from_x_y_ranges(
                border_rect.min.x..=border_rect.max.x,
                border_rect.min.y..=padding_rect.min.y,
            );
            let border_bottom_rect = Rect::from_x_y_ranges(
                border_rect.min.x..=border_rect.max.x,
                padding_rect.max.y..=border_rect.max.y,
            );
            let border_left_rect = Rect::from_x_y_ranges(
                border_rect.min.x..=padding_rect.min.x,
                border_rect.min.y..=border_rect.max.y,
            );
            let border_right_rect = Rect::from_x_y_ranges(
                padding_rect.max.x..=border_rect.max.x,
                border_rect.min.y..=border_rect.max.y,
            );
            dc.display_list.insert(
                i,
                Paint::Fill(
                    border_top_rect,
                    style.border_top_color().resolve(current_color),
                ),
            );
            dc.display_list.insert(
                i,
                Paint::Fill(
                    border_right_rect,
                    style.border_right_color().resolve(current_color),
                ),
            );
            dc.display_list.insert(
                i,
                Paint::Fill(
                    border_bottom_rect,
                    style.border_bottom_color().resolve(current_color),
                ),
            );
            dc.display_list.insert(
                i,
                Paint::Fill(
                    border_left_rect,
                    style.border_left_color().resolve(current_color),
                ),
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
        let font_size = style.font_size();
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
            font_size,
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
                let line_height = style.line_height().resolve(font_size);
                let half_leading = line_height - font_size;
                if ic.cursor.x + advance > rect.max.x {
                    // trace!(cursor = ?context.cursor, advance, max_x = rect.max.x);
                    self.flush(dc, ic)?;
                }
                ic.max_ascent = ic.max_ascent.max(ascent + half_leading);
                ic.max_height = ic.max_height.max(line_height);
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
        // move text paints for ‘vertical-align’
        for text in &mut ic.line_display_list[..] {
            match text {
                Paint::Text(rect, _, font, _) => {
                    *rect = rect.translate(vec2(
                        0.0,
                        ic.max_ascent - font.ab.ascent() / dc.viewport.scale,
                    ));
                }
                _ => unreachable!(),
            }
        }

        // move text paints for ‘text-align’
        let available = self.read().rect.width();
        let width = ic
            .line_display_list
            .iter()
            .map(|x| x.rect().right())
            .fold(0.0, f32::max);
        let offset = match self.read().text_align {
            CssTextAlign::Left => 0.0,
            CssTextAlign::Right => available - width,
            CssTextAlign::Center => (available - width) / 2.0,
        };
        for text in &mut ic.line_display_list[..] {
            match text {
                Paint::Text(rect, _, _, _) => {
                    *rect = rect.translate(vec2(offset, 0.0));
                }
                _ => unreachable!(),
            }
        }

        for text in ic.line_display_list.drain(..) {
            dc.display_list.push(text);
        }

        ic.cursor.x = ic.content_rect.min.x;
        ic.cursor.y += ic.max_height;
        ic.max_ascent = 0.0;
        ic.max_height = 0.0;

        Ok(())
    }
}
