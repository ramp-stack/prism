use crate::layout::Area;
use crate::{Context, Contract, Id};
use crate::drawable::SizedTree;

use std::path::PathBuf;

use std::fmt::Debug;
use image::RgbaImage;

use downcast_rs::{Downcast, impl_downcast};

pub type Events = std::collections::VecDeque<Box<dyn Event>>;

pub trait OnEvent {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {vec![event]}
}

//Function for event to decide on weather to pass the event to a child, Event can also be modified for the child
/// Implement the `Event` trait to allow a structure to be used in an event query.
pub trait Event: Debug + Downcast {
    /// Optionally return a clone to continue passing the event to children,
    /// or `None` to stop propagation. Can also modify the event before passing it on.
    fn pass(
        self: Box<Self>,
        _ctx: &mut Context,
        children: &[Area],
    ) -> Vec<Option<Box<dyn Event>>>;
}
impl_downcast!(Event);


/// Represents the different states of the mouse in a [`MouseEvent`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseState { 
    /// The mouse button was pressed.
    Pressed, 
    /// The mouse was moved.
    Moved, 
    /// The mouse button was released.
    Released,
    /// The mouse was scrolled.
    /// 
    /// The first value is the horizontal scroll amount (x-axis),
    /// and the second value is the vertical scroll amount (y-axis).
    Scroll(f32, f32), 
}

/// Represents the state of a keyboard key in a [`KeyboardEvent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardState {
    /// A key was pressed.
    Pressed,
    /// A key was repeated.
    Repeated,
    /// A key was released.
    Released,
}

/// # Mouse Event
///
/// `MouseEvent` is triggered whenever the [`MouseState`] changes.
/// 
/// - `position`: The mouse position at the time of the event.  
///   A component receives `Some(position)` only if the event occurred over it;  
///   otherwise, it will be `None`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MouseEvent {
    pub position: Option<(f32, f32)>,
    pub state: MouseState,
}

impl Event for MouseEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        let mut passed = false;
        children.iter().rev().map(|Area{offset, size}| {
            let position = self.position.and_then(|position| (!passed).then(|| (
                position.0 > offset.0 &&
                position.0 < offset.0+size.0 &&
                position.1 > offset.1 &&
                position.1 < offset.1+size.1
            ).then(|| {
                passed = true;
                (position.0 - offset.0, position.1 - offset.1)
            })).flatten());

            Some(Box::new(MouseEvent{position, state: self.state}) as Box<dyn Event>)
        }).collect::<Vec<_>>().into_iter().rev().collect()
    }
}

/// # Keyboard Event
///
/// `KeyboardEvent` is triggered whenever the [`KeyboardState`] changes.
/// 
/// - `key`: The [`Key`] that triggered the event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub key: Key,
    pub state: KeyboardState,
}

impl Event for KeyboardEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}
/// # Tick Event
///
/// `TickEvent` is emitted on every tick and can be used to perform continuous or repeated actions.
#[derive(Debug, Clone, Copy)]
pub struct TickEvent;
impl Event for TickEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(Box::new(*self) as Box<dyn Event>)).collect()
    }
}

#[macro_export]
macro_rules! events {
    ( $( $x:expr ),* $(,)? ) => {
        {
            vec![
                $(Box::new($x) as Box<dyn Event>),*
            ]
        }
    };
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Button {
    Pressed(bool),
    Hover(bool),
    Disable(bool),
}

impl Event for Button {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

/// Events emitted by the [`Selectable`](crate::emitters::Selectable) emmiter object.
#[derive(Debug, Clone)]
pub enum Selectable {
    Pressed(String, String),
    Selected(bool)
}

impl Event for Selectable {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

/// Events emitted by the [`Slider`](crate::emitters::Slider) emmiter object.
#[derive(Debug, Clone, Copy)]
pub enum Slider {
    Start(f32),
    Moved(f32),
}

impl Event for Slider {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

/// Events emitted by the [`TextInput`](crate::emitters::TextInput) emmiter object.
#[derive(Debug, Clone)]
pub enum TextInput {
    Hover(bool),
    Focused(bool),
    Edited(Key),
}

impl Event for TextInput {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

#[derive(Debug, Clone)]
pub enum NumericalInput {
    Delete,
    Digit(char),
    Char(char)
}

impl Event for NumericalInput {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum NamedKey {
    Enter,
    Tab,
    Space,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Delete,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Named(NamedKey),
    Character(String),
}

#[derive(Clone, Debug)]
pub struct CameraFrame(pub RgbaImage);
impl Event for CameraFrame {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

#[derive(Clone, Debug)]
pub struct PickedPhoto(pub RgbaImage);
impl Event for PickedPhoto {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

#[derive(Clone, Debug)]
pub enum Action {Replace, Add, Remove}

pub struct Update<C: Contract>(pub Id, pub PathBuf, pub Action, std::marker::PhantomData::<fn(C)>);
impl<C: Contract + 'static> Event for Update<C> {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}
impl<C: Contract> Clone for Update<C> {
    fn clone(&self) -> Self {
        Update(self.0, self.1.clone(), self.2.clone(), std::marker::PhantomData::<fn(C)>)
    }
}
impl<C: Contract> std::fmt::Debug for Update<C> {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        f.debug_tuple("Update").field(&self.0).field(&self.1).field(&self.2).finish()
    }
}
