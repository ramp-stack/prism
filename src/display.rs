use crate::drawable::{Drawable, Component};
use crate::event::{OnEvent};
use crate::layout::{Layout, Stack};
use std::collections::HashMap;

/// A container pairing a layout with a drawable element.
#[derive(Debug)]
pub struct Bin<L: Layout + 'static, D: Drawable + 'static>(pub L, pub D);

impl<L: Layout + 'static, D: Drawable + 'static> OnEvent for Bin<L, D> {}

impl<L: Layout + 'static, D: Drawable + 'static> Component for Bin<L, D> {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable> {vec![
        &mut self.1 as &mut dyn crate::drawable::Drawable,
    ]}

    fn children(&self) -> Vec<&dyn Drawable> {vec![
        &self.1 as &dyn crate::drawable::Drawable,
    ]}

    fn request_size(&self, children: Vec<crate::layout::SizeRequest>) -> crate::layout::SizeRequest {
        crate::layout::Layout::request_size(&self.0, children)
    }
    fn build(&self, size: (f32, f32), children: Vec<crate::layout::SizeRequest>) -> Vec<crate::layout::Area> {
        crate::layout::Layout::build(&self.0, size, children)
    }
}

impl<L: Layout + 'static, D: Drawable + 'static> Bin<L, D> {
    pub fn inner(&mut self) -> &mut D {
        &mut self.1
    }
    pub fn layout(&mut self) -> &mut L {
        &mut self.0
    }
}

/// A container that optionally displays a drawable item, toggling between visible and hidden states.
#[derive(Debug)]
pub struct Opt<D: Drawable + 'static>(Stack, Option<D>, Option<D>);
impl<D: Drawable + 'static> OnEvent for Opt<D> {}

impl<D: Drawable + 'static> Component for Opt<D> {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable> {vec![
        &mut self.1 as &mut dyn crate::drawable::Drawable,
    ]}

    fn children(&self) -> Vec<&dyn Drawable> {vec![&self.1 as &dyn crate::drawable::Drawable]}

    fn request_size(&self, children: Vec<crate::layout::SizeRequest>) -> crate::layout::SizeRequest {
        crate::layout::Layout::request_size(&self.0, children)
    }
    fn build(&self, size: (f32, f32), children: Vec<crate::layout::SizeRequest>) -> Vec<crate::layout::Area> {
        crate::layout::Layout::build(&self.0, size, children)
    }
}

impl<D: Drawable + 'static> Opt<D> {
    pub fn new(item: D, display: bool) -> Self {
        match display {
            true => Opt(Stack::default(), Some(item), None),
            false => Opt(Stack::default(), None, Some(item)),
        }
    }

    pub fn display(&mut self, display: bool) {
        match display {
            true if self.1.is_none() => self.1 = self.2.take(),
            false if self.2.is_none() => self.2 = self.1.take(),
            _ => {}
        }
    }

    pub fn inner(&mut self) -> &mut D {
        self.1.as_mut().unwrap_or_else(|| self.2.as_mut().unwrap())
    }

    pub fn is_showing(&self) -> bool {
        self.1.is_some()
    }
}

/// A container that holds two drawables but displays only one at a time, allowing toggling between them.
#[derive(Debug)]
pub struct EitherOr<L: Drawable + 'static, R: Drawable + 'static>(Stack, Opt<L>, Opt<R>);

impl<L: Drawable + 'static, R: Drawable + 'static> OnEvent for EitherOr<L, R> {}

impl<L: Drawable + 'static, R: Drawable + 'static> Component for EitherOr<L, R> {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable> {vec![
        &mut self.1 as &mut dyn crate::drawable::Drawable,
        &mut self.2 as &mut dyn crate::drawable::Drawable,
    ]}

    fn children(&self) -> Vec<&dyn Drawable> {vec![
        &self.1 as &dyn crate::drawable::Drawable,
        &self.2 as &dyn crate::drawable::Drawable,
    ]}

    fn request_size(&self, children: Vec<crate::layout::SizeRequest>) -> crate::layout::SizeRequest {
        crate::layout::Layout::request_size(&self.0, children)
    }
    fn build(&self, size: (f32, f32), children: Vec<crate::layout::SizeRequest>) -> Vec<crate::layout::Area> {
        crate::layout::Layout::build(&self.0, size, children)
    }
}

impl<L: Drawable + 'static, R: Drawable + 'static> EitherOr<L, R> {
    pub fn new(left: L, right: R) -> Self {
        EitherOr(Stack::default(), Opt::new(left, true), Opt::new(right, false))
    }

    pub fn display_left(&mut self, display_left: bool) {
        self.1.display(display_left);
        self.2.display(!display_left);
    }

    pub fn left(&mut self) -> &mut L { self.1.inner() }
    pub fn right(&mut self) -> &mut R { self.2.inner() }
}

/// A container that holds multiple drawables but displays only one at a time, allowing toggling between them.
#[derive(Debug)]
pub struct Enum(Stack, HashMap<String, Opt<Box<dyn Drawable>>>, String);
impl OnEvent for Enum {}

impl Component for Enum {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.1.values_mut().map(|v| v as &mut dyn crate::drawable::Drawable).collect()
    }

    fn children(&self) -> Vec<&dyn Drawable> {
        self.1.values().map(|v| v as &dyn crate::drawable::Drawable).collect()
    }

    fn request_size(&self, children: Vec<crate::layout::SizeRequest>) -> crate::layout::SizeRequest {
        crate::layout::Layout::request_size(&self.0, children)
    }
    fn build(&self, size: (f32, f32), children: Vec<crate::layout::SizeRequest>) -> Vec<crate::layout::Area> {
        crate::layout::Layout::build(&self.0, size, children)
    }
}

impl Enum {
    /// Creates a new [`Enum`] component with the given drawable items.
    /// The first item will be visible by default.
    pub fn new(items: Vec<(String, Box<dyn Drawable>)>, start: String) -> Self {
        let items = items.into_iter().map(|(name, item)| {
            (name.to_string(), Opt::new(item, name == start))
        }).collect::<Vec<(String, Opt<Box<dyn Drawable>>)>>();

        Enum(Stack::default(), items.into_iter().collect(), start.to_string())
    }

    /// Displays only the item matching the given name and hides all others. 
    /// If the key doesn't exist, defaults to the first item.
    pub fn display(&mut self, name: &str) {
        let key = match self.1.contains_key(name) { 
            true => name.to_string(),
            false => self.1.keys().next().unwrap().clone()
        };

        self.2 = key.to_string();

        for (k, v) in self.1.iter_mut() {
            v.display(*k == key);
        }
    }

    pub fn current(&self) -> String { self.2.to_string() }
    pub fn drawable(&mut self) -> &mut Opt<Box<dyn Drawable>> { 
        self.1.get_mut(&self.2).unwrap() 
    }
}
