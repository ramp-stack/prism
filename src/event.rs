use crate::layout::Area;
use crate::Context;
use crate::drawable::SizedTree;

use std::fmt::Debug;
use image::RgbaImage;

use downcast_rs::{Downcast, impl_downcast};

pub type Events = std::collections::VecDeque<Box<dyn Event>>;

pub trait OnEvent {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> { vec![event] }
}

pub trait Event: Debug + Downcast {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>>;
}
impl_downcast!(Event);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift:   bool,
    pub control: bool,
    pub alt:     bool,
    pub supermeta:    bool,
}

#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Escape, Enter, Tab, Space,
    Up, Down, Left, Right,
    Delete, Backspace, Home, End,
    Shift, Control, Alt, SuperMeta,
    CapsLock, NumLock, ScrollLock,
    Character(char)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardState{ Pressed, Repeated, Released }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton{ Left, Right, Middle }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseState {
    Pressed(MouseButton),
    Released(MouseButton),
    Scroll(f32, f32),
    Moved
}

#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent {
    pub position: Option<(f32, f32)>,
    pub state: MouseState
}

impl Event for MouseEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        let mut passed = false;
        children.iter().rev().map(|Area { offset, size }| {
            let position = self.position.and_then(|position| {
                (!passed).then(|| {
                    (position.0 > offset.0 && position.0 < offset.0 + size.0 &&
                     position.1 > offset.1 && position.1 < offset.1 + size.1)
                        .then(|| { passed = true; (position.0 - offset.0, position.1 - offset.1) })
                }).flatten()
            });
            Some(Box::new(MouseEvent { position, state: self.state}) as Box<dyn Event>)
        }).collect::<Vec<_>>().into_iter().rev().collect()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub key:       Key,
    pub state:     KeyboardState,
    pub modifiers: Modifiers,
}

#[derive(Clone, Debug)]
pub struct CameraFrame(pub RgbaImage);

#[derive(Clone, Debug)]
pub struct PickedPhoto(pub RgbaImage);

#[derive(Debug, Clone, Copy)]
pub struct TickEvent;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Button { Pressed(bool), Hover(bool), Disable(bool) }

#[derive(Debug, Clone)]
pub enum Selectable { Pressed(String, String), Selected(bool) }

#[derive(Debug, Clone, Copy)]
pub enum Slider { Start(f32), Moved(f32) }

#[derive(Debug, Clone)]
pub enum TextInput { Hover(bool), Focused(bool), Edited(Key) }

#[derive(Debug, Clone)]
pub enum NumericalInput { Delete, Digit(char), Char(char) }

macro_rules! impl_event_all_children {
    ( $( $n:ident ),* ) => {
        $(
            impl Event for $n {
                fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
                    children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
                }
            }       
        )*
    };
}
impl_event_all_children!(KeyboardEvent, CameraFrame, PickedPhoto, TickEvent, Button, Selectable, Slider, TextInput, NumericalInput);

#[macro_export]
macro_rules! events {
    ( $( $x:expr ),* $(,)? ) => {
        vec![ $(Box::new($x) as Box<dyn Event>),* ]
    };
}

//  #[derive(Clone, Debug)]
//  pub enum Action { Replace, Add, Remove }

//  pub struct Update<C: Contract>(pub Id, pub PathBuf, pub Action, std::marker::PhantomData<fn(C)>);
//  impl<C: Contract + 'static> Event for Update<C> {
//      fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
//          children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
//      }
//  }
//  impl<C: Contract> Clone for Update<C> {
//      fn clone(&self) -> Self {
//          Update(self.0, self.1.clone(), self.2.clone(), std::marker::PhantomData)
//      }
//  }
//  impl<C: Contract> std::fmt::Debug for Update<C> {
//      fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
//          f.debug_tuple("Update").field(&self.0).field(&self.1).field(&self.2).finish()
//      }
//  }
