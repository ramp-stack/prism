use crate::layout::Area;
use crate::Context;
use crate::drawable::SizedTree;

use std::fmt::Debug;
use std::sync::Arc;
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

/// Which mouse button was involved in a [`MouseState::Pressed`] or [`MouseState::Released`] event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
}

/// Represents the different states of the mouse in a [`MouseEvent`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MouseState {
    /// A mouse button was pressed. Carries which button.
    Pressed(MouseButton),
    /// The mouse was moved.
    Moved,
    /// A mouse button was released. Carries which button.
    Released(MouseButton),
    /// The mouse was scrolled.
    ///
    /// The first value is the horizontal scroll amount (x-axis),
    /// and the second value is the vertical scroll amount (y-axis).
    Scroll(f32, f32),
}

impl MouseState {
    /// Returns true if this is a left-button press.
    pub fn is_left_press(&self) -> bool {
        matches!(self, MouseState::Pressed(MouseButton::Left))
    }

    /// Returns true if this is a right-button press.
    pub fn is_right_press(&self) -> bool {
        matches!(self, MouseState::Pressed(MouseButton::Right))
    }

    /// Returns true if this is any button press.
    pub fn is_press(&self) -> bool {
        matches!(self, MouseState::Pressed(_))
    }

    /// Returns true if this is any button release.
    pub fn is_release(&self) -> bool {
        matches!(self, MouseState::Released(_))
    }
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

/// Tracks which modifier keys are currently held down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub ctrl: bool,
    pub alt: bool,
    pub meta: bool,
}

impl Modifiers {
    pub fn none() -> Self {
        Modifiers { shift: false, ctrl: false, alt: false, meta: false }
    }

    pub fn shift() -> Self {
        Modifiers { shift: true, ..Self::none() }
    }

    pub fn ctrl() -> Self {
        Modifiers { ctrl: true, ..Self::none() }
    }

    pub fn alt() -> Self {
        Modifiers { alt: true, ..Self::none() }
    }

    pub fn meta() -> Self {
        Modifiers { meta: true, ..Self::none() }
    }

    pub fn is_none(&self) -> bool {
        !self.shift && !self.ctrl && !self.alt && !self.meta
    }
}

/// # Mouse Event
///
/// `MouseEvent` is triggered whenever the [`MouseState`] changes.
///
/// - `position`: The mouse position at the time of the event.
///   A component receives `Some(position)` only if the event occurred over it;
///   otherwise, it will be `None`.
///
/// # Right-click / trackpad secondary tap
///
/// Trackpad two-finger tap and physical right-click both arrive as
/// `MouseState::Pressed(MouseButton::Right)`. Match on the button to
/// distinguish them from left clicks:
///
/// ```rust
/// if let Some(MouseEvent { state: MouseState::Pressed(MouseButton::Right), position: Some(pos) })
///     = event.downcast_ref::<MouseEvent>()
/// {
///     // show context menu at pos
/// }
/// ```
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
/// - `modifiers`: The modifier keys held at the time of the event.
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

/// Events emitted by the [`Selectable`](crate::emitters::Selectable) emitter object.
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

/// Events emitted by the [`Slider`](crate::emitters::Slider) emitter object.
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

/// Events emitted by the [`TextInput`](crate::emitters::TextInput) emitter object.
#[derive(Debug, Clone)]
pub enum TextInput {
    Hover(bool),
    Focused(bool),
    Edited(Key, Modifiers),
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
#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, PartialOrd, Ord)]
pub enum NamedKey {
    Enter,
    Tab,
    Space,
    Backspace,
    Escape,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    Home,
    End,
    PageUp,
    PageDown,
    Delete,
    Insert,
    Shift,
    Control,
    Alt,
    Meta,
    CapsLock,
    NumLock,
    ScrollLock,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Key {
    Named(NamedKey),
    Character(String),
}

#[derive(Debug, Clone)]
pub enum HardwareEvent {
    Clipboard(String),
    Camera(Arc<RgbaImage>),
    SafeArea(f32, f32, f32, f32),
}

impl Event for HardwareEvent {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}