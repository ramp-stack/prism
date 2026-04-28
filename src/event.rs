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

pub trait Event: Debug + Downcast {
    fn pass(
        self: Box<Self>,
        _ctx: &mut Context,
        children: &[Area],
    ) -> Vec<Option<Box<dyn Event>>>;
}
impl_downcast!(Event);

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseState { 
    Pressed, 
    Moved, 
    Released,
    Scroll(f32, f32), 
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardState {
    Pressed,
    Repeated,
    Released,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub meta: bool,
}

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub key: Key,
    pub state: KeyboardState,
    pub modifiers: Modifiers,
}

impl Event for KeyboardEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}

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
    Backspace,
    Home,
    End,
    Shift,
    Control,
    Alt,
    Meta,
    CapsLock,
    NumLock,
    ScrollLock,
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