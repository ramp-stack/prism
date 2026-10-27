extern crate self as prism;

pub mod drawable;
use drawable::{Area, Color, Drawable, Instruction, Item, LayoutTree, Size};
use drawable::shape::{Shape, Variant};
pub mod events;
use events::{ChildEvent, DynEvent, Event, Events, WindowEvent};
pub mod hardware;
pub mod traits;
pub mod air;
pub mod service;
pub use service::Service;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {
    /// Available size reported by the platform, excluding native keyboard obstruction.
    pub size: Size,
    pub insets: (f32, f32, f32, f32),
    /// Selects mobile layout and touch-oriented interaction behavior.
    pub is_mobile: bool,
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
    pub fn new<A: Application, H: traits::Hardware>(hardware: H, air: air::Air, window: Window) -> Self {
        let mut context = Context::new(hardware, air, window);
        let app = Box::new(A::new(&mut context));
        let mut prism = Self {app, context};
        prism.handle_events();
        prism
    }

    pub fn on_event<E: Event>(&mut self, event: E) {
        self.context.emit(event);
        self.handle_events();
    }

    pub async fn changed(&mut self) {
        let callback = self.app.as_mut().changed().await;
        callback(self.app.as_mut().as_any_mut(), &mut self.context);
        self.handle_events();
    }

    pub fn draw(&mut self) -> Vec<Instruction> {
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

    fn handle_events(&mut self) {
        while let Some(event) = self.context.events.pop_front() {
            if let Some(WindowEvent::Resized(window)) = event.downcast_ref::<WindowEvent>() {
                self.context.window = *window;
            }
            let (area, layout) = self.layout();
            let mut passed: Option<Box<dyn DynEvent>> = None;
            event.pass_dyn(&mut self.context, &mut [ChildEvent {area: &area, event: &mut passed}]);
            if let Some(event) = passed {self.app.event(&mut self.context, &layout, event);}
        }
    }

    fn layout(&mut self) -> (Area, LayoutTree) {
        let mut area = self.context.window.area();
        let (min, max) = self.app.request_width();
        let width = area.size.0.clamp(min, max);
        let (min, max) = self.app.request_height(width);
        let height = area.size.1.clamp(min, max);
        let layout = self.app.layout((width, height));
        area.size = (area.size.0.min(layout.0.0), area.size.1.min(layout.0.1));
        (area, layout)
    }
}
