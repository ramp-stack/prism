use crate::event::{self, OnEvent, Key, NamedKey, Event, TickEvent, MouseEvent, MouseState, KeyboardEvent, KeyboardState};
use crate::{events, Context, Request};
use crate::drawable::{Drawable, Component, SizedTree};
use crate::layout::Stack;
use std::time::Duration;

/// The [`Button`] emitter wraps a drawable component
/// and converts mouse input into a small set of semantic button states:
///
/// - [`Button::Pressed(true)`](crate::event::Button::Pressed) — when the mouse is pressed within the button’s bounds.
/// - [`Button::Pressed(false)`](crate::event::Button::Pressed) — when the mouse is pressed outside the button’s bounds.
/// - [`Button::Hover(true)`](crate::event::Button::Hover) — when the mouse moves over the button.
/// - [`Button::Hover(false)`](crate::event::Button::Hover) — when the mouse leaves the button.
///
/// This allows components to react to common button states without manually handling raw input.
///
#[derive(Debug, Component)]
pub struct Button<D: Drawable + 'static>(Stack, pub D);
impl<D: Drawable + 'static> Button<D> {
    pub fn new(child: D) -> Self {Button(Stack::default(), child)}
}

impl<D: Drawable + 'static> OnEvent for Button<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> { 
        if let Some(event) = event.downcast_ref::<MouseEvent>() {
            // return match event.state {
            //     MouseState::Pressed if event.position.is_some() => 
            //         events![event::Button::Pressed(true)],
            //     MouseState::Moved | MouseState::Scroll(..) if !crate::IS_MOBILE => 
            //         events![event::Button::Hover(event.position.is_some())],
            //     MouseState::Released => {
            //         match event.position.is_some() {
            //             true if !crate::IS_MOBILE => events![event::Button::Hover(true)],
            //             _ => events![event::Button::Pressed(false)],
            //         } 
            //     },
            //     _ => Vec::new()
            // };

            match event.state {
                MouseState::Pressed if event.position.is_some() => {
                    return events![event::Button::Pressed(true)];
                }
                // MouseState::Pressed if event.position.is_none() && !crate::IS_MOBILE => self.2 = false,
                MouseState::Moved | MouseState::Scroll(..) if !crate::IS_MOBILE => {
                    return events![event::Button::Hover(event.position.is_some())];
                }
                MouseState::Released => {
                    if !crate::IS_MOBILE && event.position.is_some() {
                        // true => events.push(Box::new(event::TextInput::Hover(true))),
                        return events![event::Button::Hover(true)];
                    } else {
                        return events![event::Button::Pressed(false)];
                    }
                }
                _ => {}
            }
        }
        vec![event]
    }
}

#[derive(Debug, Component)]
pub struct NumericalInput<D: Drawable + 'static>(Stack, pub D);

impl<D: Drawable + 'static> NumericalInput<D> {
    pub fn new(child: D) -> Self {
        NumericalInput(Stack::default(), child)
    }
}

impl<D: Drawable + 'static> OnEvent for NumericalInput<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(KeyboardEvent { state: KeyboardState::Pressed, key }) = event.downcast_ref::<KeyboardEvent>() {

            match key {
                Key::Named(NamedKey::Delete) => {
                    return events![event::NumericalInput::Delete];
                }
                Key::Character(c) => {
                    if let Some(ch) = c.to_string().chars().next() {
                        if ch.is_ascii_digit() { 
                            return events![event::NumericalInput::Digit(ch)]
                        }
                        if matches!(ch, '.' | '/' | ':') { 
                            return events![event::NumericalInput::Char(ch)]
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
/// When pressed, it emits an event with its unique ID and group ID, 
/// allowing other components in the same group to update their state accordingly.
///
/// - [`Selectable::Pressed(id, group_id)`](crate::event::Selectable::Pressed) - when this element was pressed,
/// - [`Selectable::Selected(true)`](crate::event::Selectable::Selected) - when this element was selected,
/// - [`Selectable::Selected(false)`](crate::event::Selectable::Selected) - when another item in the same group was selected.
#[derive(Debug, Component)]
pub struct Selectable<D: Drawable + 'static>(Stack, pub D, #[skip] uuid::Uuid, #[skip] uuid::Uuid);
impl<D: Drawable + 'static> Selectable<D> {
    pub fn new(child: D, group_id: uuid::Uuid) -> Self {
        Selectable(Stack::default(), child, uuid::Uuid::new_v4(), group_id)
    }
}
impl<D: Drawable + 'static> OnEvent for Selectable<D> {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> { 
        if let Some(MouseEvent { state: MouseState::Pressed, position: Some(_) }) = event.downcast_ref::<MouseEvent>() {
            ctx.send(Request::Event(Box::new(event::Selectable::Pressed(self.2.to_string(), self.3.to_string()))));
        } else if let Some(event::Selectable::Pressed(id, group_id)) = event.downcast_ref::<event::Selectable>() {
            if *group_id == self.3.to_string() {
                let is = *id == self.2.to_string();
                return vec![Box::new(event::Selectable::Selected(is))]
            }
        }
        vec![event]
    }
}

/// The [`Slider`] emitter wraps a drawable component
/// and converts mouse input into a small set of semantic slider states:
///
/// - [`Slider::Start(x)`](crate::event::Slider::Start) — when the user clicks or begins dragging.
/// - [`Slider::Moved(x)`](crate::event::Slider::Moved) — while dragging with the mouse pressed.
/// - Automatically stops tracking when released.
#[derive(Debug, Component)]
pub struct Slider<D: Drawable + 'static>(Stack, pub D, #[skip] bool);
impl<D: Drawable + 'static> Slider<D> {
    pub fn new(child: D) -> Self {Slider(Stack::default(), child, false)}
}

impl<D: Drawable + 'static> OnEvent for Slider<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> { 
        if let Some(MouseEvent { state, position, }) = event.downcast_ref::<MouseEvent>() {
            return match (state, position) {
                (MouseState::Pressed, Some((x, _))) => {
                    self.2 = true;
                    events![event::Slider::Start(*x)]
                },
                (MouseState::Released, _) => {
                    self.2 = false;
                    Vec::new()
                },
                (MouseState::Scroll(..) | MouseState::Moved, Some((x, _)))
                    if self.2 => {
                    events![event::Slider::Moved(*x)]
                }
                _ => Vec::new()
            };
        }
        vec![event]
    }
}

/// The [`TextInput`] emitter wraps a drawable component
/// and converts raw input into a small set of semantic text input states:
///
/// - [`TextInput::Focused(true)`](crate::event::TextInput::Focused) — when focused (clicked inside bounds).
/// - [`TextInput::Focused(false)`](crate::event::TextInput::Focused) — when unfocused (clicked outside bounds).
/// - [`TextInput::Hover(true)`](crate::event::TextInput::Hover) — when the mouse hovers over the input.
/// - [`TextInput::Hover(false)`](crate::event::TextInput::Hover) — when the mouse leaves the input.
/// - Passes keyboard events through only when focused.
#[derive(Debug, Component)]
pub struct TextInput<D: Drawable + 'static>(Stack, pub D, #[skip] bool);
impl<D: Drawable + 'static> TextInput<D> {
    pub fn new(child: D) -> Self {TextInput(Stack::default(), child, false)}
}

impl<D: Drawable + 'static> OnEvent for TextInput<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(e) = event.downcast_ref::<MouseEvent>() {
            let mut events: Vec<Box<dyn Event>> = Vec::new();

            match e.state {
                MouseState::Pressed if e.position.is_some() => {
                    self.2 = true;
                    events.push(Box::new(event::TextInput::Focused(true)));
                }
                MouseState::Pressed => {
                    if e.position.is_none() && !crate::IS_MOBILE { 
                        self.2 = false; 
                        events.push(Box::new(event::TextInput::Focused(false)));
                    }
                },
                MouseState::Moved | MouseState::Scroll(..) if !crate::IS_MOBILE && !self.2 => {
                    events.push(Box::new(event::TextInput::Hover(e.position.is_some())));
                }
                //     if !crate::IS_MOBILE && e.position.is_none() {
                //         // true => events.push(Box::new(event::TextInput::Hover(true))),
                //         events.push(Box::new(event::TextInput::Focused(false)));
                //     }
                // }
                _ => {}
            }

            events.push(event);
            return events;
        } else if let Some(KeyboardEvent { state: KeyboardState::Pressed, key: _ }) = event.downcast_ref() {
            return if self.2 { vec![event] } else { Vec::new() };
        }

        vec![event]
    }
}

#[derive(Debug, Component)]
pub struct Scrollable<D: Drawable + 'static>(Stack, pub Momentum<D>, #[skip] (f32, f32));

impl<D: Drawable + 'static> Scrollable<D> {
    pub fn new(child: D) -> Self {
        Scrollable(Stack::default(), Momentum::new(child), (0.0, 0.0))
    }
}

impl<D: Drawable + 'static> std::ops::Deref for Scrollable<D> {
    type Target = Momentum<D>;
    fn deref(&self) -> &Self::Target {&self.1}
}

impl<D: Drawable + 'static> std::ops::DerefMut for Scrollable<D> {
    fn deref_mut(&mut self) -> &mut Self::Target {&mut self.1}
}

impl<D: Drawable + 'static> OnEvent for Scrollable<D> {
    fn on_event(&mut self, _ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(MouseEvent{position: Some(position), state}) = event.downcast_ref::<event::MouseEvent>() {
            match state {
                MouseState::Pressed => {
                    self.2 = *position;
                    return Vec::new();
                },
                MouseState::Released => {
                    if (position.1 - self.2.1).abs() < 5.0 {
                        return vec![Box::new(MouseEvent{position: Some(*position), state: MouseState::Pressed}) as Box<dyn Event>];
                    }

                    return Vec::new();
                }
                _ => {}
            }
        } 

        vec![event]
    }
}

#[derive(Debug, Component)]
pub struct Momentum<D: Drawable + 'static> {
    layout: Stack,
    pub inner: D,
    #[skip] touching: bool,
    #[skip] start_touch: Option<(f32, f32)>,
    #[skip] mouse: (f32, f32),
    #[skip] scroll: Option<(f32, f32)>,
    #[skip] time: Option<Duration>,
    #[skip] speed: Option<f32>,
}

impl<D: Drawable + 'static> Momentum<D> {
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

impl<D: Drawable + 'static> OnEvent for Momentum<D> {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> { 
        if crate::IS_MOBILE {
            if let Some(MouseEvent{position: Some(position), state}) = event.downcast_ref::<MouseEvent>() {
                match state {
                    MouseState::Pressed => {
                        self.scroll = Some(*position);
                        self.scroll = Some(*position);
                        self.touching = true;
                    }, 
                    MouseState::Moved => {
                        self.mouse = *position;
                    }, 
                    MouseState::Released => {
                        self.touching = false;
                    },
                    MouseState::Scroll(..) => {
                        self.scroll = Some(*position);
                    }, 
                }
                self.mouse = *position;
            } else if event.downcast_ref::<TickEvent>().is_some() && !self.touching {
                if let Some(time) = self.time {
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
                        let state = (speed.abs() > 0.01).then_some(
                            MouseState::Scroll(0.0, speed)
                        );

                        if let Some(s) = state {
                            ctx.send(Request::Event(Box::new(MouseEvent { position: Some(self.mouse), state: s })));
                        }
                    }
                }
            }
        }
        vec![event]
    }
}


