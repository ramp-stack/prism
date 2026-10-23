extern crate self as prism;

use futures::FutureExt;

use std::io::Read;
use std::time::Duration;

pub mod drawable;
use drawable::{Area, Color, Drawable, Instruction, Item, LayoutTree, Size};
use drawable::shape::{Shape, Variant};
pub mod events;
use events::{ChildEvent, DynEvent, Event, Events, WindowEvent};
pub mod hardware;
use hardware::{Image, Media, Sound};
pub mod traits;
pub mod air;
pub mod service;
pub use service::Service;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {
    /// Available size reported by the platform, excluding the docked native keyboard.
    pub size: Size,
    pub insets: (f32, f32, f32, f32),
    pub keyboard_visible: bool,
    pub touch_controls: bool,
}
impl Window {
    /// The safe area within the available window.
    pub fn area(&self) -> Area {
        let (top, left, bottom, right) = self.insets;
        Area {
            offset: (left, top),
            size: ((self.size.0 - left - right).max(0.0), (self.size.1 - top - bottom).max(0.0)),
        }
    }
}

pub trait Application: 'static {
    fn name() -> String;
    fn new(ctx: &mut Context) -> impl Drawable;
}

pub struct Context {
    pub hardware: hardware::Hardware,
    pub air: air::Air,
    pub window: Window,
    /// Full-window fill drawn behind safe-area content. Transparent by default.
    pub background: Color,
    events: Events,
}
impl Context {
    pub fn new<H: traits::Hardware>(hardware: H, air: air::Air, window: Window) -> Self {
        Self {
            hardware: hardware::Hardware::new(hardware), air, window,
            background: Color::transparent(), events: Events::new(),
        }
    }

    pub fn emit<E: Event>(&mut self, event: E) {self.events.push_back(Box::new(event));}
    pub fn events(&mut self) -> Events {std::mem::take(&mut self.events)}
}

pub struct Prism {
    app: Box<dyn Drawable>,
    context: Context,
}
impl Prism {
    /// Register content constructors when implementing a Prism backend.
    pub const fn content_backend<I: traits::Image, M: traits::Media>() -> impl inventory::Collect {
        ContentBackend {
            read_image: |reader| I::read(reader).map(Image::from),
            image_from_rgba: |dimensions, pixels| Image::from(I::from_rgba(dimensions, pixels)),
            media: || Media::from(M::new()),
            read_media: |reader| M::read(reader).map(Media::from),
            media_from_sound: |sound| M::try_from(sound).map(Media::from),
            media_from_image: |image| M::try_from(image).map(Media::from),
        }
    }

    pub fn new<A: Application, H: traits::Hardware>(hardware: H, air: air::Air, window: Window) -> Self {
        let mut context = Context::new(hardware, air, window);
        let app = Box::new(A::new(&mut context));
        Self {app, context}
    }

    pub fn on_event<E: Event>(&mut self, event: E) {self.context.emit(event);}
    pub fn resize(&mut self, window: Window) {self.on_event(WindowEvent::Resized(window));}

    pub async fn changed(&mut self) {
        if !self.context.events.is_empty() {return}
        self.app.changed(&mut self.context).await;
    }

    pub fn draw(&mut self) -> Vec<Instruction> {
        for _ in 0..64 {
            while let Some(event) = self.context.events.pop_front() {
                if let Some(WindowEvent::Resized(window)) = event.downcast_ref::<WindowEvent>() {
                    self.context.window = *window;
                }
                let (area, layout) = self.layout();
                let mut passed: Option<Box<dyn DynEvent>> = None;
                event.pass_dyn(&mut self.context, &mut [ChildEvent {area: &area, event: &mut passed}]);
                if let Some(event) = passed {self.app.event(&mut self.context, &layout, event);}
            }
            if self.changed().now_or_never().is_none() {break}
        }
        let (area, layout) = self.layout();
        let bound = (area.offset.0, area.offset.1, area.size.0, area.size.1);
        let mut instructions = Vec::new();
        if self.context.background.3 > 0 {
            let (width, height) = self.context.window.size;
            let shape = Shape::new(width, height, self.context.background, Variant::Rectangle);
            instructions.push(Instruction {
                offset: (0.0, 0.0), bound: (0.0, 0.0, width, height), item: Item::Shape(shape),
            });
        }
        instructions.extend(self.app.draw(&layout, area.offset, bound));
        instructions
    }

    fn layout(&mut self) -> (Area, LayoutTree) {
        let mut area = self.context.window.area();
        let mut range = self.app.size_range();
        let mut layout = self.app.layout(area.size, &range);
        // Settle text wrapping before dispatching input or drawing.
        for _ in 0..4 {
            let next = self.app.size_range();
            if next == range {break}
            range = next;
            layout = self.app.layout(area.size, &range);
        }
        area.size = (area.size.0.min(layout.0.0), area.size.1.min(layout.0.1));
        (area, layout)
    }
}

struct ContentBackend {
    read_image: fn(&mut dyn Read) -> Result<Image, String>,
    image_from_rgba: fn((u32, u32), Vec<u8>) -> Image,
    media: fn() -> Media,
    read_media: fn(&mut dyn traits::Reader) -> Result<Media, String>,
    media_from_sound: fn(Sound) -> Result<Media, String>,
    media_from_image: fn((Image, Duration)) -> Result<Media, String>,
}
inventory::collect!(ContentBackend);
impl ContentBackend {
    fn get() -> &'static Self {
        let mut backends = inventory::iter::<Self>.into_iter();
        let backend = backends.next().expect("No Prism content backend registered");
        assert!(backends.next().is_none(), "Multiple Prism content backends registered");
        backend
    }
}
