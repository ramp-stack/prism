use crate::drawable::{Drawable, Component};
use crate::event::{OnEvent};
use crate::layout::{Layout, Stack};
use std::collections::HashMap;
use std::clone::Clone;

/// A container pairing a layout with a drawable element.
#[derive(Debug, Component, Clone)]
pub struct Bin<L: Layout + Clone + 'static, D: Drawable + Clone + 'static>(pub L, pub D);

impl<L: Layout + Clone + 'static, D: Drawable + Clone + 'static> OnEvent for Bin<L, D> {}

impl<L: Layout + Clone + 'static, D: Drawable + Clone + 'static> Bin<L, D> {
    pub fn inner(&mut self) -> &mut D {
        &mut self.1
    }
    pub fn get_layout(&mut self) -> &mut L {
        &mut self.0
    }
}

/// A container that optionally displays a drawable item, toggling between visible and hidden states.
#[derive(Debug, Component, Clone)]
pub struct Opt<D: Drawable + Clone + 'static>(Stack, Option<D>, #[skip] Option<D>);
impl<D: Drawable + Clone + 'static> OnEvent for Opt<D> {}

impl<D: Drawable + Clone + 'static> Opt<D> {
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

    pub fn is_showing(&self) -> bool { self.1.is_some() }
}

/// A container that holds two drawables but displays only one at a time, allowing toggling between them.
#[derive(Debug, Component, Clone)]
pub struct EitherOr<L: Drawable + Clone + 'static, R: Drawable + Clone + 'static>(Stack, Opt<L>, Opt<R>);

impl<L: Drawable + Clone + 'static, R: Drawable + Clone + 'static> OnEvent for EitherOr<L, R> {}

impl<L: Drawable + Clone + 'static, R: Drawable + Clone + 'static> EitherOr<L, R> {
    pub fn new(left: L, right: R) -> Self {
        EitherOr(Stack::default(), Opt::new(left, true), Opt::new(right, false))
    }

    pub fn display_left(&mut self, display_left: bool) {
        self.1.display(display_left);
        self.2.display(!display_left);
    }

    pub fn left(&mut self) -> &mut L { self.1.inner() }
    pub fn right(&mut self) -> &mut R { self.2.inner() }
    pub fn is_left(&self) -> bool { self.1.is_showing() }
}

/// A container that holds multiple drawables but displays only one at a time, allowing toggling between them.
#[derive(Debug, Component, Clone)]
pub struct Enum<D: Drawable + Clone + 'static>(Stack, HashMap<String, Opt<D>>, #[skip] String);
impl<D: Drawable + Clone + 'static> OnEvent for Enum<D> {}

impl<D: Drawable + Clone + 'static> Enum<D> {
    /// Creates a new [`Enum`] component, Clone with the given drawable items.
    /// The first item will be visible by default.
    pub fn new(items: Vec<(String, D)>, start: String) -> Self {
        let items = items.into_iter().map(|(name, item)| {
            (name.to_string(), Opt::new(item, name == start))
        }).collect::<Vec<(String, Opt<D>)>>();

        Enum(Stack::default(), items.into_iter().collect(), start.to_string())
    }

    /// Displays only the item matching the given name and hides all others. 
    /// If the key doesn't exist, defaults to the first item.
    pub fn display(&mut self, name: &str) {
        if self.1.contains_key(name) {  
            self.2 = name.to_string();

            for (k, v) in self.1.iter_mut() {
                v.display(*k == name);
            }
        };
    }

    pub fn current(&self) -> String { self.2.to_string() }
    pub fn drawable(&mut self) -> &mut Opt<D> { 
        self.1.get_mut(&self.2).unwrap() 
    }
}
