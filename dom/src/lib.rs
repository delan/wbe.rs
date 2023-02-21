pub mod style;

pub use crate::style::Style;

use std::{
    fmt::{Debug, Display},
    sync::{Arc, RwLock, Weak},
};

use owning_ref::{RwLockReadGuardRef, RwLockWriteGuardRefMut};
use tracing::{instrument, trace, warn};

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
    Element(String, Vec<(String, String)>, Style),
    Text(String, Style),
    Comment(String),
}

#[derive(Clone)]
pub struct Node(Arc<RwLock<OwnedNode>>);

impl Debug for Node {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            format!("{:?}", &*self.read())
                .strip_prefix("Owned")
                .unwrap()
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
            NodeData::Element(n, _, _) => {
                write!(f, "\x1B[1;36m{}(\x1B[0m", n)?;
                for (i, child) in self.children().iter().enumerate() {
                    write!(f, "{}{}", if i > 0 { " " } else { "" }, child)?;
                }
                write!(f, "\x1B[1;36m)\x1B[0m")
            }
            // NodeData::Text(x) => write!(f, "#text({:?})", x),
            NodeData::Text(x, _) => write!(f, "{:?}", x),
            // NodeData::Comment(x) => write!(f, "#comment({:?})", x),
            NodeData::Comment(x) => write!(f, "\x1B[90m<!--{:?}-->\x1B[0m", x),
        }
    }
}

impl Display for NodeData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            NodeData::Document => write!(f, "\x1B[1;36m#document\x1B[0m"),

            NodeData::Element(n, _, _) => write!(f, "\x1B[1;36m{}\x1B[0m", n),
            NodeData::Text(x, _) => write!(f, "{:?}", x),
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
        Self::new(NodeData::Element(name.to_owned(), attrs, Style::empty()))
    }

    pub fn text(value: impl ToOwned<Owned = String>) -> Self {
        Self::new(NodeData::Text(value.to_owned(), Style::empty()))
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

    pub fn data_mut(&self) -> NodeWrite<NodeData> {
        self.write().map_mut(|x| &mut x.inner)
    }

    pub fn r#type(&self) -> NodeType {
        *self.read().map(|x| match &x.inner {
            NodeData::Document => &NodeType::Document,
            NodeData::Element(_, _, _) => &NodeType::Element,
            NodeData::Text(_, _) => &NodeType::Text,
            NodeData::Comment(_) => &NodeType::Comment,
        })
    }

    pub fn name(&self) -> NodeRead<str> {
        self.read().map(|x| match &x.inner {
            NodeData::Document => "#document",
            NodeData::Element(n, _, _) => &n,
            NodeData::Text(_, _) => "#text",
            NodeData::Comment(_) => "#comment",
        })
    }

    pub fn value(&self) -> Option<NodeRead<str>> {
        self.read()
            .try_map(|x| match &x.inner {
                NodeData::Document => Err(()),
                NodeData::Element(_, _, _) => Err(()),
                NodeData::Text(text, _) => Ok(&**text),
                NodeData::Comment(text) => Ok(&**text),
            })
            .ok()
    }

    pub fn attrs(&self) -> Option<NodeRead<[(String, String)]>> {
        self.read()
            .try_map(|x| match &x.inner {
                NodeData::Element(_, attrs, _) => Ok(&**attrs),
                _ => Err(()),
            })
            .ok()
    }

    pub fn attr(&self, name: &str) -> Option<NodeRead<String>> {
        self.read()
            .try_map(|x| match &x.inner {
                NodeData::Element(_, attrs, _) => attrs
                    .iter()
                    .filter(|(n, _)| n == name)
                    .map(|(_, v)| v)
                    .next()
                    .ok_or(()),
                _ => Err(()),
            })
            .ok()
    }

    pub fn text_content(&self) -> String {
        let mut result = String::new();

        for node in self.descendants().filter(|x| x.r#type() == NodeType::Text) {
            result += &*node.value().unwrap();
        }

        result
    }

    pub fn children(&self) -> NodeRead<[Node]> {
        self.read().map(|x| &*x.children)
    }

    pub fn descendants(&self) -> impl Iterator<Item = Node> {
        NodeIterator(vec![(self.clone(), 0)])
    }
}

impl NodeData {
    pub fn style(&self) -> Style {
        match self {
            NodeData::Document => Style::empty(),
            NodeData::Element(_, _, style) => style.clone(),
            NodeData::Text(_, style) => style.clone(),
            NodeData::Comment(_) => Style::empty(),
        }
    }

    pub fn set_style(&mut self, new_style: Style) {
        match self {
            NodeData::Document => panic!(),
            NodeData::Element(_, _, style) => *style = new_style,
            NodeData::Text(_, style) => *style = new_style,
            NodeData::Comment(_) => panic!(),
        }
    }
}

struct NodeIterator(Vec<(Node, usize)>);
impl Iterator for NodeIterator {
    type Item = Node;

    fn next(&mut self) -> Option<Self::Item> {
        // e.g. (#document, 2), (html, 1), (head, 0)
        while let Some((node, i)) = self.0.last_mut() {
            // if head has a children[0]
            if *i < node.children().len() {
                // this time, return head.children[0]
                let result = node.children()[*i].clone();

                // next time, try head.children[1]
                *i += 1;

                // but actually, next time, try head.children[0].children[0]
                self.0.push((result.clone(), 0));

                return Some(result);
            }

            // we’ve run out, pop back to parent
            self.0.pop();
        }

        None
    }
}
