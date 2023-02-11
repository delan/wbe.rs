use std::sync::{Arc, RwLock};
use std::{fmt::Debug, mem::swap, str};

use egui::Vec2;

use crate::dom::{Node, NodeData, OwnedNode};
use crate::*;

#[derive(Debug)]
pub enum Document {
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
        display_list: Vec<paint::PaintText>,
        viewport: viewport::ViewportInfo,
    },
}

impl Default for Document {
    fn default() -> Self {
        Self::LaidOut {
            location: "about:blank".to_owned(),
            response_body: "".to_owned(),
            dom: Node::document(),
            display_list: vec![],
            viewport: Default::default(),
        }
    }
}

impl Document {
    pub fn take(&mut self) -> Self {
        let mut result = Self::None;
        swap(self, &mut result);

        result
    }

    pub fn invalidate_layout(self) -> Self {
        match self {
            Document::LaidOut {
                location,
                response_body,
                dom,
                ..
            } => Document::Parsed {
                location,
                response_body,
                dom,
            },
            other => other,
        }
    }

    pub fn status(&self) -> &'static str {
        match self {
            Document::None => "None",
            Document::Navigated { .. } => "Navigated",
            Document::Loaded { .. } => "Loaded",
            Document::Parsed { .. } => "Parsed",
            Document::LaidOut { .. } => "LaidOut",
        }
    }

    pub fn size(&self) -> Vec2 {
        let mut result = Vec2::ZERO;
        if let Self::LaidOut { display_list, .. } = self {
            for paint in display_list {
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
}
