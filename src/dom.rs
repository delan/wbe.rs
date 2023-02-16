use std::{
    fmt::{Debug, Display},
    sync::{Arc, RwLock, Weak},
};

use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use tracing::{instrument, trace};

use crate::*;

pub type NodeRead<'n, T> = RwLockReadGuardRef<'n, OwnedNode, T>;
pub type NodeWrite<'n, T> = RwLockWriteGuardRefMut<'n, OwnedNode, T>;

#[derive(Debug)]
pub struct OwnedNode {
    pub parent: Weak<RwLock<OwnedNode>>,
    pub children: Vec<Node>,
    pub inner: NodeData,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum NodeType {
    Document,
    Element,
    Text,
    Comment,
}

#[derive(Debug, Clone)]
pub enum NodeData {
    Document,
    Element(String, Vec<(String, String)>),
    Text(String),
    Comment(String),
}

#[derive(Clone)]
pub struct Node(Arc<RwLock<OwnedNode>>);

impl Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            format!("{:?}", self.read()).strip_prefix("Owned").unwrap()
        )
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &*self.data() {
            NodeData::Document => {
                write!(f, "\x1B[1;36m#document(\x1B[0m")?;
                for (i, child) in self.children().iter().enumerate() {
                    write!(f, "{}{}", if i > 0 { " " } else { "" }, child)?;
                }
                write!(f, "\x1B[1;36m)\x1B[0m")
            }
            NodeData::Element(n, _) => {
                write!(f, "\x1B[1;36m{}(\x1B[0m", n)?;
                for (i, child) in self.children().iter().enumerate() {
                    write!(f, "{}{}", if i > 0 { " " } else { "" }, child)?;
                }
                write!(f, "\x1B[1;36m)\x1B[0m")
            }
            // NodeData::Text(x) => write!(f, "#text({:?})", x),
            NodeData::Text(x) => write!(f, "{:?}", x),
            // NodeData::Comment(x) => write!(f, "#comment({:?})", x),
            NodeData::Comment(x) => write!(f, "\x1B[90m<!--{:?}-->\x1B[0m", x),
        }
    }
}

impl Display for NodeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeData::Document => write!(f, "\x1B[1;36m#document\x1B[0m"),

            NodeData::Element(n, _) => write!(f, "\x1B[1;36m{}\x1B[0m", n),
            NodeData::Text(x) => write!(f, "{:?}", x),
            NodeData::Comment(x) => write!(f, "\x1B[90m<!--{:?}-->\x1B[0m", x),
        }
    }
}

impl Node {
    pub fn new(inner: NodeData) -> Self {
        Self(Arc::new(RwLock::new(OwnedNode {
            parent: Weak::new(),
            children: vec![],
            inner,
        })))
    }

    pub fn document() -> Self {
        Self::new(NodeData::Document)
    }

    pub fn element(name: impl ToOwned<Owned = String>, attrs: Vec<(String, String)>) -> Self {
        Self::new(NodeData::Element(name.to_owned(), attrs))
    }

    pub fn text(value: impl ToOwned<Owned = String>) -> Self {
        Self::new(NodeData::Text(value.to_owned()))
    }

    pub fn comment(value: impl ToOwned<Owned = String>) -> Self {
        Self::new(NodeData::Comment(value.to_owned()))
    }

    #[instrument(skip(self, children))]
    pub fn append(&self, children: &[Node]) -> Self {
        for child in children {
            trace!(%self, %child);
            child.write().parent = Arc::downgrade(&self.0);
            self.write().children.push(child.clone());
            trace!(%self, %child);
        }

        self.clone()
    }

    #[instrument(skip(self))]
    pub fn parent(&self) -> Option<Self> {
        self.read().parent.upgrade().map(Self)
    }

    pub fn read(&self) -> NodeRead<OwnedNode> {
        NodeRead::new(self.0.read().unwrap())
    }

    pub fn write(&self) -> NodeWrite<OwnedNode> {
        NodeWrite::new(self.0.write().unwrap())
    }

    pub fn data(&self) -> NodeRead<NodeData> {
        self.read().map(|x| &x.inner)
    }

    pub fn r#type(&self) -> NodeType {
        *self.read().map(|x| match &x.inner {
            NodeData::Document => &NodeType::Document,
            NodeData::Element(_, _) => &NodeType::Element,
            NodeData::Text(_) => &NodeType::Text,
            NodeData::Comment(_) => &NodeType::Comment,
        })
    }

    pub fn name(&self) -> NodeRead<str> {
        self.read().map(|x| match &x.inner {
            NodeData::Document => "#document",
            NodeData::Element(n, _) => &n,
            NodeData::Text(_) => "#text",
            NodeData::Comment(_) => "#comment",
        })
    }

    pub fn value(&self) -> Option<NodeRead<str>> {
        self.read()
            .try_map(|x| match &x.inner {
                NodeData::Document => Err(()),
                NodeData::Element(_, _) => Err(()),
                NodeData::Text(text) => Ok(&**text),
                NodeData::Comment(text) => Ok(&**text),
            })
            .ok()
    }

    pub fn children(&self) -> NodeRead<[Node]> {
        self.read().map(|x| &*x.children)
    }
}
