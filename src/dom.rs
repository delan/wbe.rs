use std::{
    fmt::{Debug, Display},
    sync::{Arc, RwLock, Weak},
};

use tracing::{instrument, trace};

use crate::*;

#[derive(Debug)]
pub struct OwnedNode {
    pub parent: Weak<RwLock<OwnedNode>>,
    pub children: Vec<Node>,
    pub inner: NodeData,
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
            format!("{:?}", r!(self.0)).strip_prefix("Owned").unwrap()
        )
    }
}

impl Display for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &r!(self.0).inner {
            NodeData::Document => {
                write!(f, "\x1B[1;36m#document(\x1B[0m")?;
                for (i, child) in r!(self.0).children.iter().enumerate() {
                    write!(f, "{}{}", if i > 0 { " " } else { "" }, child)?;
                }
                write!(f, "\x1B[1;36m)\x1B[0m")
            }
            NodeData::Element(n, _) => {
                write!(f, "\x1B[1;36m{}(\x1B[0m", n)?;
                for (i, child) in r!(self.0).children.iter().enumerate() {
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
            w!(child.0).parent = Arc::downgrade(&self.0);
            w!(self.0).children.push(child.clone());
            trace!(%self, %child);
        }

        self.clone()
    }

    #[instrument(skip(self))]
    pub fn parent(&self) -> Option<Self> {
        // trace!(%self, parent = %r!(self.0).parent.upgrade().map(Self).unwrap());
        r!(self.0).parent.upgrade().map(Self)
    }

    pub fn data(&self) -> NodeData {
        r!(self.0).inner.clone()
    }

    pub fn name(&self) -> String {
        match &r!(self.0).inner {
            NodeData::Document => "#document".to_owned(),
            NodeData::Element(n, _) => n.clone(),
            NodeData::Text(_) => "#text".to_owned(),
            NodeData::Comment(_) => "#comment".to_owned(),
        }
    }

    pub fn value(&self) -> Option<String> {
        match &r!(self.0).inner {
            NodeData::Document => None,
            NodeData::Element(_, _) => None,
            NodeData::Text(x) => Some(x.clone()),
            NodeData::Comment(x) => Some(x.clone()),
        }
    }

    // FIXME https://stackoverflow.com/a/63523617
    pub fn children(&self) -> Vec<Node> {
        r!(self.0).children.clone()
    }

    pub fn ancestors_inclusive(&self) -> Vec<Node> {
        let mut result = vec![self.clone()];
        let mut node = self.clone();
        while let Some(parent) = node.parent() {
            result.push(parent.clone());
            node = parent;
        }

        result
    }
}
