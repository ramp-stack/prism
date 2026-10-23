use downcast_rs::{Downcast, impl_downcast};
use futures::{FutureExt, StreamExt};
use futures::future::LocalBoxFuture;
use futures::stream::FuturesUnordered;
use futures_timer::Delay;
pub use inventory;
pub use prism_proc::{on_change, on_event};

use std::any::{Any, TypeId};
use std::fmt::Debug;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use std::collections::{BTreeMap, VecDeque};

use crate::{Context, Window};
use crate::drawable::{Area, Component, Drawable};

pub type Events = std::collections::VecDeque<Box<dyn DynEvent>>;
pub type ChangeFuture<'a> = LocalBoxFuture<'a, Change>;

/// Registration emitted by `#[on_change]` in the component's crate.
pub struct ChangeHandler {
    pub component: fn() -> TypeId,
    pub field: &'static str,
    pub listen: for<'a> fn(&'a mut dyn Any) -> ChangeFuture<'a>,
}
impl ChangeHandler {
    pub fn find<C: Component>(field: &str) -> Option<&'static Self> {
        let mut handlers = inventory::iter::<Self>.into_iter()
            .filter(|handler| (handler.component)() == TypeId::of::<C>() && handler.field == field);
        let handler = handlers.next()?;
        assert!(handlers.next().is_none(), "a component field can have only one change listener");
        Some(handler)
    }
}
inventory::collect!(ChangeHandler);

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

pub struct ChildEvent<'a, E> {
    pub area: &'a Area,
    pub event: &'a mut Option<E>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifiers {
    pub shift: bool,
    pub control: bool,
    pub alt: bool,
    pub supermeta: bool,
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
pub struct MouseEvent {
    pub position: Option<(f32, f32)>,
    pub state: MouseState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyboardEvent {
    pub key: Key,
    pub state: KeyboardState,
    pub modifiers: Modifiers,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Notification {
    pub header: String,
    pub body: String,
}
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowEvent {Resized(Window), Focused(bool), Moved(i32, i32)}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEvent {Dropped(PathBuf), Hovered(PathBuf), Cancelled}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorEvent {
    pub device: u64,
    pub entered: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaEvent {Play, Pause, VolumeUp, VolumeDown}

#[diagnostic::on_unimplemented(message = "add #[on_event] to this OnEvent implementation")]
pub trait EventRegistration<E: Event> {}

pub trait OnEvent<E: Event>: Component + EventRegistration<E> {
    fn on(&mut self, ctx: &mut Context, event: E) -> Vec<Box<dyn DynEvent>>;
}

pub trait OnChange<U, Tag>: Component {
    fn on_change(&mut self, ctx: &mut Context, update: U);
}

pub trait ChangeSource: 'static {
    type Update: 'static;
    fn next(&mut self) -> impl Future<Output = Option<Self::Update>>;

    fn changed(&mut self) -> impl Future<Output = Self::Update> {
        async move {
            match self.next().await {
                Some(update) => update,
                None => std::future::pending().await,
            }
        }
    }
}
impl ChangeSource for Timer {
    type Update = ();
    async fn next(&mut self) -> Option<()> {
        let Some((_, delay)) = &mut self.pending else {return std::future::pending().await};
        delay.await;
        self.pending = None;
        Some(())
    }
}
impl<S: ChangeSource> ChangeSource for Option<S> {
    type Update = S::Update;
    async fn next(&mut self) -> Option<Self::Update> {
        match self {Some(source) => source.next().await, None => None}
    }
}
impl<K: Clone + Ord + 'static, S: ChangeSource> ChangeSource for BTreeMap<K, S> {
    type Update = Vec<(K, S::Update)>;
    async fn next(&mut self) -> Option<Self::Update> {
        let mut waits: FuturesUnordered<_> = self.iter_mut().map(|(key, source)| async move {
            (key.clone(), source.next().await)
        }).collect();
        let mut updates = Vec::new();
        while let Some((key, update)) = waits.next().await {
            if let Some(update) = update {updates.push((key, update)); break}
        }
        while let Some(Some((key, update))) = waits.next().now_or_never() {
            if let Some(update) = update {updates.push((key, update));}
        }
        updates.sort_by(|a, b| a.0.cmp(&b.0));
        (!updates.is_empty()).then_some(updates)
    }
}
impl<S: ChangeSource> ChangeSource for VecDeque<S> {
    type Update = S::Update;
    async fn next(&mut self) -> Option<Self::Update> {
        while let Some(source) = self.front_mut() {
            if let Some(update) = source.next().await {return Some(update)}
            self.pop_front();
        }
        None
    }
}

pub trait Event: Debug + Sized + 'static {
    fn pass(&self, ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]);
}

impl Event for Notification {
    fn pass(&self, _ctx: &mut Context, children: &mut [ChildEvent<'_, Self>]) {
        for child in children {*child.event = Some(self.clone());}
    }
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

impl Event for MediaEvent {
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

/// A cancelable one-shot change source, inactive by default.
/// Consuming an update disarms the timer; only an explicit reset schedules another.
/// Clones keep the deadline but can be awaited, reset, and canceled independently.
#[derive(Debug, Default)]
pub struct Timer {
    pending: Option<(Instant, Delay)>,
}
impl Timer {
    pub fn new(duration: Duration) -> Self {
        let mut timer = Self::default();
        timer.reset(duration);
        timer
    }

    pub fn reset(&mut self, duration: Duration) {
        self.pending = Some((Instant::now() + duration, Delay::new(duration)));
    }

    pub fn cancel(&mut self) {self.pending = None;}
    pub fn is_active(&self) -> bool {self.pending.is_some()}
}
impl Clone for Timer {
    fn clone(&self) -> Self {
        Self {
            pending: self.pending.as_ref().map(|(at, _)| {
                (*at, Delay::new(at.saturating_duration_since(Instant::now())))
            }),
        }
    }
}

pub struct Change(Box<dyn FnOnce(&mut dyn Drawable, &mut Context)>);
impl Change {
    pub fn new(apply: impl FnOnce(&mut dyn Drawable, &mut Context) + 'static) -> Self {
        Self(Box::new(apply))
    }
    pub async fn select(mut changes: Vec<ChangeFuture<'_>>) -> Self {
        match changes.len() {
            0 => return std::future::pending().await,
            1 => return changes.pop().unwrap().await,
            _ => {},
        }
        fastrand::shuffle(&mut changes);
        let mut waits: FuturesUnordered<_> = changes.into_iter().collect();
        waits.next().await.unwrap()
    }
    pub fn apply(self, drawable: &mut dyn Drawable, ctx: &mut Context) {(self.0)(drawable, ctx)}
    pub fn child<C: Component>(self, index: usize) -> Self {
        Self::new(move |drawable, ctx| {
            let children = &mut drawable.downcast_mut::<C>().unwrap().children_mut();
            self.apply(children[index], ctx);
        })
    }
    pub fn map<C: Drawable>(self, access: fn(&mut C) -> &mut dyn Drawable) -> Self {
        Self::new(move |drawable, ctx| {
            self.apply(access(drawable.downcast_mut::<C>().unwrap()), ctx);
        })
    }
}
