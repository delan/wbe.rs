use std::fmt::{Debug, Display};

use egui::Color32;

use tracing::warn;
use wbe_core::FONT_SIZE;
use wbe_css_parser::{color_numeric, font_shorthand, CssLength};

lazy_static::lazy_static! {
    pub static ref INITIAL_STYLE: Style = Style {
        display: Some("inline".to_owned()),
        margin: CssQuad::one(CssLength::Zero),
        padding: CssQuad::one(CssLength::Zero),
        border: CssQuad::one(CssBorder::none()),
        font: Some(CssFont::initial()),
        width: Some(CssWidth::Auto),
        background_color: Some(CssColor::Other(Color32::TRANSPARENT)),
        color: Some(Color32::BLACK),
    };
}

#[derive(Debug, Clone)]
pub struct Style {
    pub display: Option<String>,
    pub margin: CssQuad<CssLength>,
    pub padding: CssQuad<CssLength>,
    pub border: CssQuad<CssBorder>,
    pub font: Option<CssFont>,
    pub width: Option<CssWidth>,
    pub background_color: Option<CssColor>,
    pub color: Option<Color32>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssColor {
    CurrentColor,
    Other(Color32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssQuad<T: Debug + Clone + PartialEq> {
    top: Option<T>,
    right: Option<T>,
    bottom: Option<T>,
    left: Option<T>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssDisplay {
    None,
    Inline,
    Block,
    InlineBlock,
    ListItem,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CssFont {
    pub line_height: Option<f32>,
    pub size: Option<f32>,
    pub family: Option<Vec<String>>,
    pub style: Option<CssFontStyle>,
    pub weight: Option<CssFontWeight>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssFontStyle {
    Normal,
    Italic,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssFontWeight {
    Normal,
    Bold,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CssWidth {
    Auto,
    Length(CssLength),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CssBorder {
    pub width: Option<CssLength>,
    pub color: Option<CssColor>,
}

impl Style {
    pub fn empty() -> Self {
        Self {
            display: None,
            margin: CssQuad::default(),
            padding: CssQuad::default(),
            border: CssQuad::default(),
            font: None,
            width: None,
            background_color: None,
            color: None,
        }
    }

    pub fn initial() -> &'static Self {
        &*INITIAL_STYLE
    }

    pub fn new_inherited(&self) -> Self {
        Self {
            font: self.font.clone(),
            color: self.color.clone(),
            ..Self::initial().clone()
        }
    }

    pub fn apply(&mut self, other: &Style) {
        self.display = other.display.clone().or(self.display.clone());
        self.background_color = other
            .background_color
            .clone()
            .or(self.background_color.clone());
        self.color = other.color.clone().or(self.color.clone());
    }

    pub fn display(&self) -> CssDisplay {
        match &**self
            .display
            .as_ref()
            .unwrap_or_else(|| Self::initial().display.as_ref().unwrap())
        {
            "none" => CssDisplay::None,
            "inline" => CssDisplay::Inline,
            "block" => CssDisplay::Block,
            _ => CssDisplay::Inline,
        }
    }

    pub fn margin(&self) -> &CssQuad<CssLength> {
        &self.margin
    }

    pub fn margin_mut(&mut self) -> &mut CssQuad<CssLength> {
        &mut self.margin
    }

    pub fn margin_top(&self) -> CssLength {
        *self.margin.top_unwrap_or(&Self::initial().margin)
    }

    pub fn margin_right(&self) -> CssLength {
        *self.margin.right_unwrap_or(&Self::initial().margin)
    }

    pub fn margin_bottom(&self) -> CssLength {
        *self.margin.bottom_unwrap_or(&Self::initial().margin)
    }

    pub fn margin_left(&self) -> CssLength {
        *self.margin.left_unwrap_or(&Self::initial().margin)
    }

    pub fn padding(&self) -> &CssQuad<CssLength> {
        &self.padding
    }

    pub fn padding_mut(&mut self) -> &mut CssQuad<CssLength> {
        &mut self.padding
    }

    pub fn padding_top(&self) -> CssLength {
        *self.padding.top_unwrap_or(&Self::initial().padding)
    }

    pub fn padding_right(&self) -> CssLength {
        *self.padding.right_unwrap_or(&Self::initial().padding)
    }

    pub fn padding_bottom(&self) -> CssLength {
        *self.padding.bottom_unwrap_or(&Self::initial().padding)
    }

    pub fn padding_left(&self) -> CssLength {
        *self.padding.left_unwrap_or(&Self::initial().padding)
    }

    pub fn border(&self) -> &CssQuad<CssBorder> {
        &self.border
    }

    pub fn border_mut(&mut self) -> &mut CssQuad<CssBorder> {
        &mut self.border
    }

    pub fn border_width(&self) -> CssQuad<CssLength> {
        self.border.map(|x| x.width)
    }

    pub fn border_top_width(&self) -> CssLength {
        self.border.top_map_or(&Self::initial().border, |b| b.width)
    }

    pub fn border_top_color(&self) -> CssColor {
        self.border.top_map_or(&Self::initial().border, |b| b.color)
    }

    pub fn border_right_width(&self) -> CssLength {
        self.border
            .right_map_or(&Self::initial().border, |b| b.width)
    }

    pub fn border_right_color(&self) -> CssColor {
        self.border
            .right_map_or(&Self::initial().border, |b| b.color)
    }

    pub fn border_bottom_width(&self) -> CssLength {
        self.border
            .bottom_map_or(&Self::initial().border, |b| b.width)
    }

    pub fn border_bottom_color(&self) -> CssColor {
        self.border
            .bottom_map_or(&Self::initial().border, |b| b.color)
    }

    pub fn border_left_width(&self) -> CssLength {
        self.border
            .left_map_or(&Self::initial().border, |b| b.width)
    }

    pub fn border_left_color(&self) -> CssColor {
        self.border
            .left_map_or(&Self::initial().border, |b| b.color)
    }

    pub fn font_size(&self) -> f32 {
        let result = self.get(|s| s.font.as_ref().map(|f| f.size));

        result.unwrap_or_else(|| Self::initial().font.as_ref().unwrap().size.unwrap())
    }

    pub fn font_style(&self) -> CssFontStyle {
        let result = self.get(|s| s.font.as_ref().map(|f| f.style));

        result.unwrap_or_else(|| Self::initial().font.as_ref().unwrap().style.unwrap())
    }

    pub fn font_weight(&self) -> CssFontWeight {
        let result = self.get(|s| s.font.as_ref().map(|f| f.weight));

        result.unwrap_or_else(|| Self::initial().font.as_ref().unwrap().weight.unwrap())
    }

    pub fn box_width(&self, percent_base: f32) -> f32 {
        let font_size = self.font_size();
        match self.get(|s| s.width) {
            CssWidth::Auto => percent_base,
            CssWidth::Length(x) => {
                let margin_left = self
                    .margin
                    .left_unwrap_or(&Self::initial().margin)
                    .resolve(percent_base, font_size);
                let padding_left = self
                    .padding
                    .left_unwrap_or(&Self::initial().padding)
                    .resolve(percent_base, font_size);
                let border_left = self
                    .border_left_width()
                    .resolve_no_percent(font_size)
                    .unwrap_or(0.0);
                let border_right = self
                    .border_right_width()
                    .resolve_no_percent(font_size)
                    .unwrap_or(0.0);
                let padding_right = self
                    .padding
                    .right_unwrap_or(&Self::initial().padding)
                    .resolve(percent_base, font_size);
                let margin_right = self
                    .margin
                    .right_unwrap_or(&Self::initial().margin)
                    .resolve(percent_base, font_size);

                x.resolve(percent_base, font_size)
                    + margin_left
                    + padding_left
                    + border_left
                    + border_right
                    + padding_right
                    + margin_right
            }
        }
    }

    pub fn background_color(&self) -> CssColor {
        self.get(|s| s.background_color)
    }

    pub fn color(&self) -> Color32 {
        self.get(|s| s.color)
    }

    fn get<T>(&self, getter: impl Fn(&Self) -> Option<T>) -> T {
        getter(self).unwrap_or_else(|| getter(Self::initial()).unwrap())
    }
}

impl CssColor {
    pub fn parse(value: &str) -> Option<CssColor> {
        fn rgba(rgba32: u32) -> Color32 {
            Color32::from_rgba_unmultiplied(
                (rgba32 >> 24) as _,
                (rgba32 >> 16 & 255) as _,
                (rgba32 >> 8 & 255) as _,
                (rgba32 & 255) as _,
            )
        }

        if value.eq_ignore_ascii_case("currentcolor") {
            return Some(CssColor::CurrentColor);
        }

        Some(CssColor::Other(match value {
            // impl defined in CSS1, defined in CSS2
            x if x.eq_ignore_ascii_case("maroon") => rgba(0x800000ff),
            x if x.eq_ignore_ascii_case("red") => rgba(0xff0000ff),
            x if x.eq_ignore_ascii_case("yellow") => rgba(0xffff00ff),
            x if x.eq_ignore_ascii_case("olive") => rgba(0x808000ff),
            x if x.eq_ignore_ascii_case("purple") => rgba(0x800080ff),
            x if x.eq_ignore_ascii_case("fuchsia") => rgba(0xff00ffff),
            x if x.eq_ignore_ascii_case("white") => rgba(0xffffffff),
            x if x.eq_ignore_ascii_case("lime") => rgba(0x00ff00ff),
            x if x.eq_ignore_ascii_case("green") => rgba(0x008000ff),
            x if x.eq_ignore_ascii_case("navy") => rgba(0x000080ff),
            x if x.eq_ignore_ascii_case("blue") => rgba(0x0000ffff),
            x if x.eq_ignore_ascii_case("aqua") => rgba(0x00ffffff),
            x if x.eq_ignore_ascii_case("teal") => rgba(0x008080ff),
            x if x.eq_ignore_ascii_case("black") => rgba(0x000000ff),
            x if x.eq_ignore_ascii_case("silver") => rgba(0xc0c0c0ff),
            x if x.eq_ignore_ascii_case("gray") => rgba(0x808080ff),

            // not defined in CSS1, defined in CSS2
            x if x.eq_ignore_ascii_case("orange") => rgba(0xffA50000),

            // defined after CSS2
            x if x.eq_ignore_ascii_case("transparent") => rgba(0x00000000),
            x if x.eq_ignore_ascii_case("rebeccapurple") => rgba(0x663399FF),

            other => {
                if let Ok(("", result)) = color_numeric(other) {
                    result
                } else {
                    warn!("unknown color {:?}", other);
                    return None;
                }
            }
        }))
    }

    pub fn resolve(&self, current_color: Color32) -> Color32 {
        match self {
            CssColor::CurrentColor => current_color,
            CssColor::Other(other) => *other,
        }
    }
}

impl<T: Debug + Clone + PartialEq> CssQuad<T> {
    pub fn one(all: impl Into<Option<T>>) -> Self {
        let all = all.into();

        Self::four(all.clone(), all.clone(), all.clone(), all)
    }

    pub fn two(vertical: impl Into<Option<T>>, horizontal: impl Into<Option<T>>) -> Self {
        let vertical = vertical.into();
        let horizontal = horizontal.into();

        Self::four(vertical.clone(), horizontal.clone(), vertical, horizontal)
    }

    pub fn three(
        top: impl Into<Option<T>>,
        horizontal: impl Into<Option<T>>,
        bottom: impl Into<Option<T>>,
    ) -> Self {
        let horizontal = horizontal.into();

        Self::four(top, horizontal.clone(), bottom, horizontal)
    }

    pub fn four(
        top: impl Into<Option<T>>,
        right: impl Into<Option<T>>,
        bottom: impl Into<Option<T>>,
        left: impl Into<Option<T>>,
    ) -> Self {
        Self {
            top: top.into(),
            right: right.into(),
            bottom: bottom.into(),
            left: left.into(),
        }
    }

    pub fn top_unwrap(&self) -> &T {
        self.top.as_ref().unwrap()
    }

    pub fn right_unwrap(&self) -> &T {
        self.right.as_ref().unwrap()
    }

    pub fn bottom_unwrap(&self) -> &T {
        self.bottom.as_ref().unwrap()
    }

    pub fn left_unwrap(&self) -> &T {
        self.left.as_ref().unwrap()
    }

    pub fn top_unwrap_or(&self, initial: &'static Self) -> &T {
        self.top.as_ref().unwrap_or(initial.top.as_ref().unwrap())
    }

    pub fn right_unwrap_or(&self, initial: &'static Self) -> &T {
        self.right.as_ref().unwrap_or(initial.top.as_ref().unwrap())
    }

    pub fn bottom_unwrap_or(&self, initial: &'static Self) -> &T {
        self.bottom
            .as_ref()
            .unwrap_or(initial.top.as_ref().unwrap())
    }

    pub fn left_unwrap_or(&self, initial: &'static Self) -> &T {
        self.left.as_ref().unwrap_or(initial.top.as_ref().unwrap())
    }

    pub fn top_mut(&mut self, initial: &'static Self) -> &mut T {
        if self.top.is_none() {
            self.top = initial.top.clone();
        }

        self.top.as_mut().unwrap()
    }

    pub fn right_mut(&mut self, initial: &'static Self) -> &mut T {
        if self.right.is_none() {
            self.right = initial.right.clone();
        }

        self.right.as_mut().unwrap()
    }

    pub fn bottom_mut(&mut self, initial: &'static Self) -> &mut T {
        if self.bottom.is_none() {
            self.bottom = initial.bottom.clone();
        }

        self.bottom.as_mut().unwrap()
    }

    pub fn left_mut(&mut self, initial: &'static Self) -> &mut T {
        if self.left.is_none() {
            self.left = initial.left.clone();
        }

        self.left.as_mut().unwrap()
    }

    pub fn map<U: Debug + Clone + PartialEq>(&self, f: impl Fn(&T) -> Option<U>) -> CssQuad<U> {
        CssQuad::four(
            self.top.as_ref().map(&f).flatten(),
            self.right.as_ref().map(&f).flatten(),
            self.bottom.as_ref().map(&f).flatten(),
            self.left.as_ref().map(&f).flatten(),
        )
    }

    pub fn map_or<U: Debug + Clone + PartialEq>(
        &self,
        initial: &Self,
        f: impl Fn(&T) -> Option<U>,
    ) -> CssQuad<U> {
        CssQuad::four(
            self.top
                .as_ref()
                .map_or_else(|| f(initial.top_unwrap()), &f),
            self.right
                .as_ref()
                .map_or_else(|| f(initial.right_unwrap()), &f),
            self.bottom
                .as_ref()
                .map_or_else(|| f(initial.bottom_unwrap()), &f),
            self.left
                .as_ref()
                .map_or_else(|| f(initial.left_unwrap()), &f),
        )
    }

    pub fn top_map_or<U: Debug + Clone + PartialEq>(
        &self,
        initial: &Self,
        f: impl Fn(&T) -> Option<U>,
    ) -> U {
        self.top
            .as_ref()
            .map(&f)
            .flatten()
            .unwrap_or_else(|| f(initial.top.as_ref().unwrap()).unwrap())
    }

    pub fn right_map_or<U: Debug + Clone + PartialEq>(
        &self,
        initial: &Self,
        f: impl Fn(&T) -> Option<U>,
    ) -> U {
        self.right
            .as_ref()
            .map(&f)
            .flatten()
            .unwrap_or_else(|| f(initial.right.as_ref().unwrap()).unwrap())
    }

    pub fn bottom_map_or<U: Debug + Clone + PartialEq>(
        &self,
        initial: &Self,
        f: impl Fn(&T) -> Option<U>,
    ) -> U {
        self.bottom
            .as_ref()
            .map(&f)
            .flatten()
            .unwrap_or_else(|| f(initial.bottom.as_ref().unwrap()).unwrap())
    }

    pub fn left_map_or<U: Debug + Clone + PartialEq>(
        &self,
        initial: &Self,
        f: impl Fn(&T) -> Option<U>,
    ) -> U {
        self.left
            .as_ref()
            .map(&f)
            .flatten()
            .unwrap_or_else(|| f(initial.left.as_ref().unwrap()).unwrap())
    }

    pub fn parse_shorthand(value: &str, parser: impl Fn(&str) -> Option<T>) -> Option<Self> {
        let value = value.split_ascii_whitespace().collect::<Vec<_>>();
        let result = match value[..] {
            [a] => Self::one(parser(a)),
            [a, b] => Self::two(parser(a), parser(b)),
            [a, b, c] => Self::three(parser(a), parser(b), parser(c)),
            [a, b, c, d] => Self::four(parser(a), parser(b), parser(c), parser(d)),
            _ => return None,
        };

        result.require_all_valid()
    }

    fn require_all_valid(self) -> Option<Self> {
        if self.top.is_none()
            || self.right.is_none()
            || self.bottom.is_none()
            || self.left.is_none()
        {
            return None;
        }

        Some(self)
    }
}

impl<T: Debug + Copy + Clone + PartialEq> Default for CssQuad<T> {
    fn default() -> Self {
        Self {
            top: None,
            right: None,
            bottom: None,
            left: None,
        }
    }
}

impl CssFont {
    fn initial() -> Self {
        Self {
            line_height: Some(1.25),
            size: Some(FONT_SIZE),
            family: Some(vec!["serif".to_owned()]),
            style: Some(CssFontStyle::Normal),
            weight: Some(CssFontWeight::Normal),
        }
    }

    pub fn none() -> Self {
        Self {
            line_height: None,
            size: None,
            family: None,
            style: None,
            weight: None,
        }
    }

    pub fn parse_shorthand(value: &str) -> Option<(Self, CssLength)> {
        let mut result = Self::none();
        result.style = Some(CssFontStyle::Normal);
        result.weight = Some(CssFontWeight::Normal);

        if let Ok(("", (keywords, size, line_height, family))) = font_shorthand(value) {
            for keyword in keywords {
                match keyword {
                    "normal" => continue,
                    "italic" => result.style = Some(CssFontStyle::Italic),
                    "bold" => result.weight = Some(CssFontWeight::Bold),
                    _ => return None,
                }
            }
            result.line_height = line_height;
            result.family = Some(family.into_iter().map(|x| x.to_owned()).collect());
            return Some((result, size));
        }

        None
    }
}

impl CssWidth {
    pub fn resolve(&self, percent_base: f32, em_base: f32) -> f32 {
        match self {
            CssWidth::Auto => percent_base,
            CssWidth::Length(x) => x.resolve(percent_base, em_base),
        }
    }
}

impl CssBorder {
    pub fn none() -> CssBorder {
        Self {
            width: Some(CssLength::Zero),
            color: Some(CssColor::Other(Color32::from_rgb(255, 0, 255))),
        }
    }

    pub fn parse_shorthand(value: &str) -> Option<Self> {
        let mut result = Self::none();

        for value in value.split_ascii_whitespace() {
            match value {
                "0" => result.width = Some(CssLength::Zero),
                "solid" => {}
                other => {
                    if let Some(other) = CssLength::parse(other) {
                        result.width = Some(other);
                    } else if let Some(other) = CssColor::parse(other) {
                        result.color = Some(other);
                    } else {
                        return None;
                    }
                }
            }
        }

        Some(result)
    }
}

#[test]
pub fn parse() {
    assert_eq!(
        CssBorder::parse_shorthand("1em solid black"),
        Some(CssBorder {
            width: Some(CssLength::Em(1.0)),
            color: Some(CssColor::Other(Color32::BLACK))
        })
    );
}

impl Display for CssWidth {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Length(x) => write!(f, "{}", x),
        }
    }
}

impl Display for CssColor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::CurrentColor => write!(f, "currentColor"),
            Self::Other(x) => write!(f, "#{:02x}{:02x}{:02x}{:02x}", x.r(), x.g(), x.b(), x.a()),
        }
    }
}

impl Display for CssBorder {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if let Some(x) = self.width {
            write!(f, "{}", x)
        } else {
            write!(f, "unset")
        }?;
        write!(f, " ")?;
        if let Some(x) = self.color {
            write!(f, "{}", x)
        } else {
            write!(f, "unset")
        }?;

        Ok(())
    }
}

impl<T: Debug + Clone + PartialEq + Display> Display for CssQuad<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut t = f.debug_tuple("quad");
        if let Some(x) = self.top.as_ref() {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }
        if let Some(x) = self.right.as_ref() {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }
        if let Some(x) = self.bottom.as_ref() {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }
        if let Some(x) = self.left.as_ref() {
            t.field(&format_args!("{}", x));
        } else {
            t.field(&format_args!("unset"));
        }

        t.finish()
    }
}
