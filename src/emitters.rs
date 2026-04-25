use crate::event::{self, OnEvent, Key, NamedKey, Event, TickEvent, MouseEvent, MouseState, MouseButton, KeyboardEvent, KeyboardState, Modifiers};
use crate::{events, Context, Request};
use crate::drawable::{Drawable, Component, SizedTree};
use crate::layout::Stack;
use std::time::Duration;

const TEXT_INPUT_UUID: uuid::Uuid = uuid::uuid!("123e4567-e89b-12d3-a456-426614174000");

/// The [`Button`] emitter wraps a drawable component
/// and converts mouse input into a small set of semantic button states:
///
/// - [`Button::Pressed(true)`](crate::event::Button::Pressed) — when the left mouse button is pressed within bounds.
/// - [`Button::Pressed(false)`](crate::event::Button::Pressed) — when the left mouse button is released.
/// - [`Button::Hover(true)`](crate::event::Button::Hover) — when the mouse moves over the button.
/// - [`Button::Hover(false)`](crate::event::Button::Hover) — when the mouse leaves the button.
///
/// Right-click and middle-click pass through unchanged so parent components
/// can handle context menus or other secondary actions.
#[derive(Debug, Component, Clone)]
pub struct Button<D: Drawable + Clone + 'static>(Stack, pub D, #[skip] bool);
impl<D: Drawable + Clone + 'static> Button<D> {
    pub fn new(child: D) -> Self {Button(Stack::default(), child, false)}
}

impl<D: Drawable + Clone + 'static> OnEvent for Button<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(event) = event.downcast_ref::<MouseEvent>() {
            match event.state {
                // Only left-click triggers Pressed(true)
                MouseState::Pressed(MouseButton::Left) if event.position.is_some() => {
                    self.2 = true;
                    return events![event::Button::Pressed(true)];
                },
                MouseState::Moved | MouseState::Scroll(..) if !crate::IS_MOBILE => {
                    return events![event::Button::Hover(event.position.is_some())];
                },
                // Only release of a left-click triggers Pressed(false)
                MouseState::Released(MouseButton::Left) => {
                    let result = match !crate::IS_MOBILE && event.position.is_some() {
                        true if self.2 => events![event::Button::Pressed(false), event::Button::Hover(true)],
                        true => events![event::Button::Hover(true)],
                        false if self.2 => events![event::Button::Pressed(false)],
                        false => vec![]
                    };
                    self.2 = false;
                    return result;
                },
                // Right/middle clicks and other states pass through unchanged
                _ => {}
            }
        }
        vec![event]
    }
}

#[derive(Debug, Component, Clone)]
pub struct NumericalInput<D: Drawable + Clone + 'static>(Stack, pub D);

impl<D: Drawable + Clone + 'static> NumericalInput<D> {
    pub fn new(child: D) -> Self {
        NumericalInput(Stack::default(), child)
    }
}

impl<D: Drawable + Clone + 'static> OnEvent for NumericalInput<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(KeyboardEvent { state: KeyboardState::Pressed | KeyboardState::Repeated, key, .. }) = event.downcast_ref::<KeyboardEvent>() {
            match key {
                Key::Named(NamedKey::Delete | NamedKey::Backspace) => {
                    return events![event::NumericalInput::Delete];
                }
                Key::Character(c) => {
                    if let Some(ch) = c.chars().next() {
                        if ch.is_ascii_digit() {
                            return events![event::NumericalInput::Digit(ch)];
                        }
                        if matches!(ch, '.' | '/' | ':') {
                            return events![event::NumericalInput::Char(ch)];
                        }
                    }
                }
                _ => {}
            }
        }
        vec![event]
    }
}

/// The [`Selectable`] emitter allows one item in a group to be active at a time.
/// Only left-clicks trigger selection — right-clicks pass through so parent
/// components can show context menus without accidentally changing selection.
#[derive(Debug, Component, Clone)]
pub struct Selectable<D: Drawable + Clone + 'static>(Stack, pub D, #[skip] uuid::Uuid, #[skip] uuid::Uuid);
impl<D: Drawable + Clone + 'static> Selectable<D> {
    pub fn new(child: D, group_id: uuid::Uuid) -> Self {
        Selectable(Stack::default(), child, uuid::Uuid::new_v4(), group_id)
    }
}
impl<D: Drawable + Clone + 'static> OnEvent for Selectable<D> {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(MouseEvent { state: MouseState::Pressed(MouseButton::Left), position: Some(_) }) = event.downcast_ref::<MouseEvent>() {
            ctx.send(Request::Event(Box::new(event::Selectable::Pressed(self.2.to_string(), self.3.to_string()))));
        } else if let Some(event::Selectable::Pressed(id, group_id)) = event.downcast_ref::<event::Selectable>()
        && *group_id == self.3.to_string() {
            let is = *id == self.2.to_string();
            return vec![Box::new(event::Selectable::Selected(is))];
        }
        vec![event]
    }
}

/// The [`Slider`] emitter wraps a drawable component
/// and converts mouse input into a small set of semantic slider states:
///
/// - [`Slider::Start(x)`](crate::event::Slider::Start) — when the user left-clicks or begins dragging.
/// - [`Slider::Moved(x)`](crate::event::Slider::Moved) — while dragging with the left button pressed.
/// - Automatically stops tracking when released.
///
/// Right-click passes through unchanged.
#[derive(Debug, Component, Clone)]
pub struct Slider<D: Drawable + Clone + 'static>(Stack, pub D, #[skip] bool);
impl<D: Drawable + Clone + 'static> Slider<D> {
    pub fn new(child: D) -> Self {Slider(Stack::default(), child, false)}
}

impl<D: Drawable + Clone + 'static> OnEvent for Slider<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(MouseEvent { state, position }) = event.downcast_ref::<MouseEvent>() {
            return match (state, position) {
                (MouseState::Pressed(MouseButton::Left), Some((x, _))) => {
                    self.2 = true;
                    events![event::Slider::Start(*x)]
                },
                (MouseState::Released(MouseButton::Left), _) => {
                    self.2 = false;
                    Vec::new()
                },
                (MouseState::Scroll(..) | MouseState::Moved, Some((x, _))) if self.2 => {
                    events![event::Slider::Moved(*x)]
                }
                _ => return vec![event],
            };
        }
        vec![event]
    }
}

#[derive(Debug, Component, Clone)]
pub struct TextInput<D: Drawable + Clone + 'static>(Stack, pub D, #[skip] Option<bool>);
impl<D: Drawable + Clone + 'static> TextInput<D> {
    pub fn new(child: D, requires_focus: bool) -> Selectable<Self> {
        Selectable::new(TextInput(Stack::default(), child, requires_focus.then_some(false)), TEXT_INPUT_UUID)
    }
}

impl<D: Drawable + Clone + 'static> OnEvent for TextInput<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(event::Selectable::Selected(selected)) = event.downcast_ref::<event::Selectable>() {
            if let Some(focus) = &mut self.2 { *focus = *selected; }
            return vec![Box::new(event::TextInput::Focused(*selected)), event];
        } else if let Some(e) = event.downcast_ref::<MouseEvent>() {
            let mut events: Vec<Box<dyn Event>> = Vec::new();
            match e.state {
                MouseState::Pressed(MouseButton::Left) if e.position.is_some() => {
                    if let Some(focus) = &mut self.2 { *focus = true; }
                    events.push(Box::new(event::TextInput::Focused(true)));
                }
                MouseState::Pressed(MouseButton::Left) if e.position.is_none() && !crate::IS_MOBILE => {
                    if let Some(focus) = &mut self.2 { *focus = false; }
                    events.push(Box::new(event::TextInput::Focused(false)));
                },
                MouseState::Moved | MouseState::Scroll(..) if !crate::IS_MOBILE && !self.2.unwrap_or_default() => {
                    events.push(Box::new(event::TextInput::Hover(e.position.is_some())));
                }
                _ => {}
            }
            events.push(event);
            return events;
        } else if let Some(KeyboardEvent { state: KeyboardState::Pressed | KeyboardState::Repeated, key, modifiers }) = event.downcast_ref::<KeyboardEvent>() {
            let key = key.clone();
            let modifiers = *modifiers;

            let focused = self.2.unwrap_or(true);
            if !focused { return Vec::new(); }

            return vec![event, Box::new(event::TextInput::Edited(key, modifiers))];
        }

        vec![event]
    }
}

#[derive(Debug, Component, Clone)]
pub struct Scrollable<D: Drawable + Clone + PartialEq + 'static>(Stack, pub Momentum<D>, #[skip] (f32, f32));

impl<D: Drawable + Clone + PartialEq + 'static> Scrollable<D> {
    pub fn new(child: D) -> Self {
        Scrollable(Stack::default(), Momentum::new(child), (0.0, 0.0))
    }
}

impl<D: Drawable + Clone + PartialEq + 'static> std::ops::Deref for Scrollable<D> {
    type Target = Momentum<D>;
    fn deref(&self) -> &Self::Target { &self.1 }
}

impl<D: Drawable + Clone + PartialEq + 'static> std::ops::DerefMut for Scrollable<D> {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.1 }
}

impl<D: Drawable + Clone + PartialEq + 'static> OnEvent for Scrollable<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(MouseEvent { position: Some(position), state }) = event.downcast_ref::<event::MouseEvent>() {
            match state {
                MouseState::Pressed(MouseButton::Left) => {
                    self.2 = *position;
                    return Vec::new();
                },
                MouseState::Released(MouseButton::Left) => {
                    if (position.1 - self.2.1).abs() < 5.0 {
                        return vec![
                            Box::new(MouseEvent { position: Some(*position), state: MouseState::Pressed(MouseButton::Left) }),
                            Box::new(MouseEvent { position: Some(*position), state: MouseState::Released(MouseButton::Left) }),
                        ];
                    }
                    return Vec::new();
                }
                _ => {}
            }
        }
        vec![event]
    }
}

#[derive(Debug, Component, Clone)]
pub struct Momentum<D: Drawable + Clone + 'static> {
    layout: Stack,
    pub inner: D,
    #[skip] touching: bool,
    #[skip] start_touch: Option<(f32, f32)>,
    #[skip] mouse: (f32, f32),
    #[skip] scroll: Option<(f32, f32)>,
    #[skip] time: Option<Duration>,
    #[skip] speed: Option<f32>,
}

impl<D: Drawable + Clone + 'static> Momentum<D> {
    pub fn new(child: D) -> Self {
        Momentum {
            layout: Stack::default(),
            inner: child,
            touching: false,
            start_touch: None,
            mouse: (0.0, 0.0),
            scroll: None,
            time: None,
            speed: None,
        }
    }
}

impl<D: Drawable + Clone + 'static> OnEvent for Momentum<D> {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if crate::IS_MOBILE {
            if let Some(MouseEvent { position: Some(position), state }) = event.downcast_ref::<MouseEvent>() {
                match state {
                    MouseState::Pressed(_) => {
                        self.scroll = Some(*position);
                        self.touching = true;
                    },
                    MouseState::Moved => {
                        self.mouse = *position;
                    },
                    MouseState::Released(_) => {
                        self.touching = false;
                    },
                    MouseState::Scroll(..) => {
                        self.scroll = Some(*position);
                    },
                }
                self.mouse = *position;
            } else if event.downcast_ref::<TickEvent>().is_some() && !self.touching && let Some(time) = self.time {
                match &mut self.speed {
                    Some(speed) => {
                        *speed *= 0.92;
                        if speed.abs() < 0.1 {
                            self.time = None;
                            self.speed = None;
                            self.start_touch = None;
                            return vec![event];
                        }
                    }
                    None => {
                        let start_y = self.start_touch.unwrap_or((0.0, 0.0)).1;
                        let end_y = self.scroll.unwrap_or((0.0, 0.0)).1;
                        let y_traveled = end_y - start_y;
                        let time_secs = time.as_secs_f32();
                        self.speed = Some(-((y_traveled / time_secs) * 0.05));
                    }
                }

                if let Some(speed) = self.speed {
                    if let Some(s) = (speed.abs() > 0.01).then_some(MouseState::Scroll(0.0, speed)) {
                        ctx.send(Request::Event(Box::new(MouseEvent { position: Some(self.mouse), state: s })));
                    }
                }
            }
        }
        vec![event]
    }
}