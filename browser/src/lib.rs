pub mod document;

pub use crate::document::{Document, OwnedDocument};

use std::sync::{Arc, RwLock};

use backtrace::Backtrace;
use egui::Vec2;
use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use wbe_core::dump_backtrace;
use wbe_layout::ViewportInfo;

#[derive(Clone)]
pub struct Browser(Arc<RwLock<OwnedBrowser>>);

pub type BrowserRead<'n, T> = RwLockReadGuardRef<'n, OwnedBrowser, T>;
pub type BrowserWrite<'n, T> = RwLockWriteGuardRefMut<'n, OwnedBrowser, T>;

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum RenderStatus {
    Load,
    Parse,
    Layout,
    Done,
}

impl Browser {
    pub fn wrap(inner: OwnedBrowser) -> Self {
        Self(Arc::new(RwLock::new(inner)))
    }

    pub fn read(&self) -> BrowserRead<OwnedBrowser> {
        if option_env!("WBE_DEBUG_RWLOCK").is_some() {
            dump_backtrace(Backtrace::new());
        }
        BrowserRead::new(self.0.read().unwrap())
    }

    pub fn write(&self) -> BrowserWrite<OwnedBrowser> {
        if option_env!("WBE_DEBUG_RWLOCK").is_some() {
            dump_backtrace(Backtrace::new());
        }
        BrowserWrite::new(self.0.write().unwrap())
    }

    pub fn location(&self) -> BrowserRead<str> {
        self.read().map(|x| &*x.location)
    }

    pub fn location_mut(&self) -> BrowserWrite<String> {
        self.write().map_mut(|x| &mut x.location)
    }

    pub fn set_status(&self, status: RenderStatus) {
        self.write().status = status;
    }
}

pub struct OwnedBrowser {
    pub location: String,
    pub document: Document,
    pub next_document: Document,
    pub viewport: ViewportInfo,
    pub scroll: Vec2,
    pub status: RenderStatus,
    pub first_update: bool,
}

impl Default for OwnedBrowser {
    fn default() -> Self {
        Self {
            location: Default::default(),
            document: Default::default(),
            next_document: Default::default(),
            viewport: Default::default(),
            scroll: Vec2::ZERO,
            status: RenderStatus::Done,
            first_update: true,
        }
    }
}
