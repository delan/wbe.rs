#![feature(array_chunks)]
#![feature(iter_array_chunks)]

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
use tracing::{debug, trace};
use unicode_segmentation::UnicodeSegmentation;

use wbe_core::{dump_backtrace, FONTS, FONT_SIZE, MARGIN};
use wbe_dom::{Node, NodeData, NodeType, Style};
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
    pub node: Node,
    pub parent: Weak<RwLock<OwnedLayout>>,
    pub previous: Weak<RwLock<OwnedLayout>>,
    pub children: Vec<Layout>,
    pub mode: LayoutMode,
    pub display_list: Vec<Paint>,
    pub rect: Rect,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutMode {
    Document,
    Block,
    Inline,
}

struct DocumentContext<'v, 'p> {
    viewport: &'v ViewportInfo,
    display_list: &'p mut Vec<Paint>,
    block: BlockContext,
}

#[derive(Debug, Clone)]
struct BlockContext {
    font_size: f32,
    font_weight_bold: bool,
    font_style_italic: bool,

    style: Style,
}

#[derive(Debug)]
struct InlineContext {
    cursor: Pos2,
    max_ascent: f32,
    max_height: f32,
    line_display_list: Vec<Paint>,

    style: Style,
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
                    if DISPLAY_BLOCK.contains(&&*child.name()) {
                        return Some(LayoutMode::Block);
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

    pub fn block(&self, node: Node) -> Self {
        Self::with_mode(node, LayoutMode::Block)
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

    pub fn display_list(&self) -> LayoutRead<[Paint]> {
        self.read().map(|x| &*x.display_list)
    }

    pub fn layout(&self, viewport: &ViewportInfo) -> eyre::Result<()> {
        assert!(self.mode() == LayoutMode::Document);

        let mut display_list = vec![];
        let mut document_context = DocumentContext {
            viewport,
            display_list: &mut display_list,
            block: BlockContext {
                font_size: FONT_SIZE,
                font_weight_bold: false,
                font_style_italic: false,
                style: Style::default(),
            },
        };

        self.layout0(&mut document_context)?;
        self.write().display_list = display_list;

        Ok(())
    }

    fn layout0(&self, dc: &mut DocumentContext) -> eyre::Result<()> {
        // trace!(mode = ?self.mode(), node = %*self.node().data());

        let initial_rect = |previous: Option<&Layout>| {
            let mut result = self.read().rect;
            if let Some(previous) = previous {
                result.set_top(previous.read().rect.bottom());
            }
            result.set_height(0.0);
            result
        };

        let old_block_context = dc.block.clone();

        // separate let releases RwLock read!
        let node = self.node().clone();
        match &*node.name() {
            // presentational hints
            x if DISPLAY_NONE.contains(&x) => return Ok(()),
            "body" => {
                // hack for body{margin:1em}
                self.write().rect.min.x += MARGIN;
                self.write().rect.max.x -= MARGIN;
                self.write().rect.min.y += MARGIN;
                self.write().rect.max.y += MARGIN;
            }
            "h1" => {
                dc.block.font_size *= 2.5;
                dc.block.font_weight_bold = true;
            }
            "h2" => {
                dc.block.font_size *= 2.0;
                dc.block.font_weight_bold = true;
            }
            "h3" => {
                dc.block.font_size *= 1.5;
                dc.block.font_weight_bold = true;
            }
            "h4" => {
                dc.block.font_size *= 1.25;
                dc.block.font_weight_bold = true;
            }
            "h5" => {
                dc.block.font_size *= 1.0;
                dc.block.font_weight_bold = true;
            }
            "h6" => {
                dc.block.font_size *= 0.75;
                dc.block.font_weight_bold = true;
            }
            _ => {}
        }
        dc.block.style = Style::new_inherited(&dc.block.style);
        dc.block.style.apply(&node.data().style());

        match self.mode() {
            LayoutMode::Document => {
                self.write().rect =
                    Rect::from_min_size(dc.viewport.rect.min, vec2(dc.viewport.rect.width(), 0.0));

                let layout = self.block(self.node().clone());
                layout.write().rect = initial_rect(None);
                layout.layout0(dc)?;

                // setting max rather than adding layout rect size (for hack)
                self.write().rect.max = layout.read().rect.max;

                // hack for body{margin:1em}
                self.write().rect.max.y += MARGIN;

                self.append(layout);
                debug!(mode = ?self.mode(), height = self.read().rect.height(), display_list_len = dc.display_list.len());
            }
            LayoutMode::Block => {
                // separate let releases RwLock read!
                let node = self.node().clone();
                match Self::mode_for(node) {
                    Some(LayoutMode::Block) => {
                        let i = dc.display_list.len();

                        // temporary layout list releases RwLock read!
                        let mut layouts: Vec<Layout> = vec![];
                        for child in &*self.node().children() {
                            let layout = self.block(child.clone());
                            layout.write().rect = initial_rect(layouts.last());
                            layout.layout0(dc)?;
                            layouts.push(layout);
                        }
                        for layout in layouts {
                            // setting max rather than adding layout rect size (for hack)
                            self.write().rect.max = layout.read().rect.max;

                            self.append(layout);
                        }

                        dc.display_list.insert(
                            i,
                            Paint::Fill(self.read().rect, dc.block.style.get_background_color()),
                        );
                    }
                    Some(LayoutMode::Inline) => {
                        let i = dc.display_list.len();

                        let mut inline_context = InlineContext {
                            cursor: self.read().rect.min,
                            max_ascent: 0.0,
                            max_height: 0.0,
                            line_display_list: vec![],

                            style: Style::new_inherited(&dc.block.style),
                        };

                        // separate let releases RwLock read!
                        let node = self.node().clone();
                        self.recurse(node, dc, &mut inline_context)?;
                        self.flush(dc, &mut inline_context)?;
                        self.write().rect.set_bottom(inline_context.cursor.y);

                        dc.display_list.insert(
                            i,
                            Paint::Fill(self.read().rect, dc.block.style.get_background_color()),
                        );
                    }
                    _ => unreachable!(),
                }
            }
            LayoutMode::Inline => unreachable!(),
        }

        dc.block = old_block_context;

        trace!(mode = ?self.mode(), node = %*self.node().data(), outer = ?self.mode(), inner = ?Self::mode_for(self.node().clone()), height = self.read().rect.height());

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
                let old_style = ic.style.clone();
                ic.style = node.data().style();
                self.open_tag(&node.name(), dc, ic);
                for child in &*node.children() {
                    self.recurse(child.clone(), dc, ic)?;
                }
                self.close_tag(&node.name(), dc, ic);
                ic.style = old_style;
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
        let font = FontInfo::new(
            FontFamily::Name(
                match (dc.block.font_weight_bold, dc.block.font_style_italic) {
                    (false, false) => FONTS[0].0.into(),
                    (true, false) => FONTS[1].0.into(),
                    (false, true) => FONTS[2].0.into(),
                    (true, true) => FONTS[3].0.into(),
                },
            ),
            match (dc.block.font_weight_bold, dc.block.font_style_italic) {
                (false, false) => FONTS[0].1,
                (true, false) => FONTS[1].1,
                (false, true) => FONTS[2].1,
                (true, true) => FONTS[3].1,
            },
            dc.block.font_size,
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
                ic.line_display_list
                    .push(Paint::Fill(rect, ic.style.get_background_color()));
                ic.line_display_list.push(Paint::Text(
                    rect,
                    ic.style.get_color(),
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
        for [mut fill, mut text] in ic.line_display_list.drain(..).array_chunks::<2>() {
            let font = match &mut text {
                Paint::Text(ref mut rect, _, font, _) => {
                    *rect = rect.translate(vec2(
                        0.0,
                        ic.max_ascent - font.ab.ascent() / dc.viewport.scale,
                    ));
                    font
                }
                _ => todo!(),
            };
            match &mut fill {
                Paint::Fill(ref mut rect, _) => {
                    *rect = rect.translate(vec2(
                        0.0,
                        ic.max_ascent - font.ab.ascent() / dc.viewport.scale,
                    ));
                }
                _ => todo!(),
            }
            dc.display_list.push(fill);
            dc.display_list.push(text);
        }

        ic.cursor.x = self.read().rect.min.x;
        ic.cursor.y += ic.max_height;
        ic.max_ascent = 0.0;
        ic.max_height = 0.0;

        Ok(())
    }

    fn open_tag(&self, name: &str, dc: &mut DocumentContext, _ic: &mut InlineContext) {
        match name {
            "b" => dc.block.font_weight_bold = true,
            "i" => dc.block.font_style_italic = true,
            "big" => dc.block.font_size *= 1.5,
            "small" => dc.block.font_size /= 1.5,
            _ => {}
        }
    }

    fn close_tag(&self, name: &str, dc: &mut DocumentContext, _ic: &mut InlineContext) {
        match name {
            "b" => dc.block.font_weight_bold = false,
            "i" => dc.block.font_style_italic = false,
            "big" => dc.block.font_size /= 1.5,
            "small" => dc.block.font_size *= 1.5,
            _ => {}
        }
    }
}
