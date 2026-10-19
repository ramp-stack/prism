use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::path::PathBuf;

use crate::{Context, Window};
use crate::drawable::{Area, Component};
use downcast_rs::{Downcast, impl_downcast};

pub use prism_proc::on_event;

pub type Events = std::collections::VecDeque<Box<dyn DynEvent>>;

pub struct ChildEvent<'a, E> { pub area: &'a Area, pub event: &'a mut Option<E> }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool, pub control: bool,
    pub alt: bool, pub supermeta: bool
}

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Escape, Enter, Tab, Space, Up, Down, Left, Right,
    Delete, Backspace, Home, End, Shift, Control, Alt, SuperMeta,
    CapsLock, NumLock, ScrollLock, Character(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardState { Pressed, Repeated, Released }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton { Left, Right, Middle }

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseState { Pressed(MouseButton), Released(MouseButton), Scroll(f32, f32), Moved }

#[derive(Debug, Clone, PartialEq)]
pub struct MouseEvent { pub position: Option<(f32, f32)>, pub state: MouseState }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent { pub key: Key, pub state: KeyboardState, pub modifiers: Modifiers }

#[derive(Debug, Clone, Copy)]
pub struct TickEvent;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowEvent {Resized(Window), Focused(bool), Moved(i32, i32)}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {Dropped(PathBuf), Hovered(PathBuf), Cancelled}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorEvent {pub device: u64, pub entered: bool}

pub trait OnEvent<E: Event>: Component + EventRegistration<E> {
    fn on(&mut self, ctx: &mut Context, event: E) -> Vec<Box<dyn DynEvent>>;
}

pub trait Event: Debug + Sized + 'static {
    fn pass(&self, ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]);
}

impl Event for MouseEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        let mut passed = false;
        for child in children.iter_mut().rev() {
            let offset = child.area.offset;
            let size = child.area.size;
            let position = self.position.and_then(|position| {
                (!passed).then(|| {
                    (position.0 >= offset.0 && position.0 < offset.0 + size.0 &&
                     position.1 >= offset.1 && position.1 < offset.1 + size.1).then(|| {
                        passed = true;
                        (position.0 - offset.0, position.1 - offset.1)
                    })
                }).flatten()
            });
            *child.event = Some(MouseEvent {position, state: self.state});
        }
    }
}

impl Event for KeyboardEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

impl Event for TickEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

impl Event for WindowEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(*self);}
    }
}

impl Event for FileEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
}

impl Event for CursorEvent {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(*self);}
    }
}

pub trait DynEvent: Debug + Downcast {
    fn pass_dyn(&self, ctx: &mut Context, children: &mut [ChildEvent<'_, Box<dyn DynEvent>>]);
}
impl_downcast!(DynEvent);

impl<E: Event> DynEvent for E {
    fn pass_dyn(&self, ctx: &mut Context, children: &mut [ChildEvent<'_, Box<dyn DynEvent>>]) {
        let mut events: Vec<Option<E>> = children.iter().map(|_| None).collect();
        let mut slots: Vec<_> = children.iter().zip(&mut events)
            .map(|(child, event)| ChildEvent {area: child.area, event}).collect();
        self.pass(ctx, &mut slots);
        for (child, event) in children.iter_mut().zip(events) {
            *child.event = event.map(|event| Box::new(event) as Box<dyn DynEvent>);
        }
    }
}

#[macro_export]
macro_rules! events {
    ( $( $x:expr ),* $(,)? ) => {
        vec![ $(Box::new($x) as Box<dyn $crate::events::DynEvent>),* ]
    };
}

//  #[derive(Debug, Clone, Copy, Eq, PartialEq)]
//  pub enum Button { Pressed(bool), Hover(bool), Disable(bool) }
//  impl_event_all_children!(Button);

//  #[derive(Debug, Clone)]
//  pub enum Selectable { Pressed(String, String), Selected(bool) }
//  impl_event_all_children!(Selectable);

//  #[derive(Debug, Clone, Copy)]
//  pub enum Slider { Start(f32), Moved(f32) }
//  impl_event_all_children!(Slider);

//  #[derive(Debug, Clone)]
//  pub enum TextInput { Hover(bool), Focused(bool), Edited(Key) }
//  impl_event_all_children!(TextInput);

//  #[derive(Debug, Clone)]
//  pub enum NumericalInput { Delete, Digit(char), Char(char) }
//  impl_event_all_children!(NumericalInput);

//  #[derive(Debug, Clone)]
//  pub enum Scrollable { ScrollTo(f32) }
//  impl_event_all_children!(Scrollable);

//  macro_rules! impl_event_all_children {
//      ( $( $n:ident ),* ) => {
//          $(
//              impl Event for $n {
//                  fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
//                      for child in children {*child.event = Some(self.clone());}
//                  }
//              }
//          )*
//      };
//  }


#[doc(hidden)]
pub use inventory;

#[doc(hidden)]
#[diagnostic::on_unimplemented(message = "add #[on_event] to this OnEvent implementation")]
pub trait EventRegistration<E: Event> {}

#[doc(hidden)]
pub struct EventHandler {
    pub types: fn() -> (TypeId, TypeId),
    pub call: fn(&mut dyn Any, &mut Context, Box<dyn DynEvent>) -> Vec<Box<dyn DynEvent>>,
}
impl EventHandler {
    pub fn dispatch<C: Component>(component: &mut C, ctx: &mut Context, event: Box<dyn DynEvent>) -> Vec<Box<dyn DynEvent>> {
        let types = (TypeId::of::<C>(), (*event).type_id());
        match inventory::iter::<Self>.into_iter().find(|handler| (handler.types)() == types) {
            Some(handler) => (handler.call)(component, ctx, event),
            None => vec![event],
        }
    }
}
inventory::collect!(EventHandler);
