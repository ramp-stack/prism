extern crate self as prism;

use std::marker::PhantomData;
use drawable::{Area, Drawable, Instruction, LayoutTree, Size};
use events::{ChildEvent, DynEvent, Event, Events, TickEvent, WindowEvent};

pub mod drawable;
pub mod events;
pub mod hardware;
pub mod traits;
pub mod air;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Window {pub size: Size, pub insets: (f32, f32, f32, f32)}
impl Window {
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
    fn contracts<A: traits::Air>(contracts: air::Contracts<A>) -> air::Contracts<A> {contracts}
    fn new(ctx: &mut Context) -> impl Drawable;
}

pub struct Context {
    pub hardware: hardware::Hardware,
    pub air: air::Air,
    pub window: Window,
    events: Events,
}
impl Context {
    pub fn new<A: Application, H: traits::Hardware, T: traits::Air>(hardware: H, air: T, window: Window) -> Self {
        Self {
            hardware: hardware::Hardware::new(hardware), air: air::Air::new::<A, T>(air),
            window, events: Events::new(),
        }
    }
    pub fn emit<E: Event>(&mut self, event: E) {self.events.push_back(Box::new(event));}
    pub fn events(&mut self) -> Events {std::mem::take(&mut self.events)}
}

pub struct Prism<A: Application> {
    app: Box<dyn Drawable>,
    context: Context,
    application: PhantomData<A>,
}
impl<A: Application> Prism<A> {
    pub fn new<H: traits::Hardware, T: traits::Air>(hardware: H, air: T, window: Window) -> Self {
        let mut context = Context::new::<A, H, T>(hardware, air, window);
        let app = Box::new(A::new(&mut context));
        Self {app, context, application: PhantomData}
    }

    pub fn on_event<E: Event>(&mut self, event: E) {self.context.emit(event);}
    pub fn resize(&mut self, window: Window) {self.on_event(WindowEvent::Resized(window));}

    pub fn draw(&mut self) -> Vec<Instruction> {
        self.context.emit(TickEvent);
        while let Some(event) = self.context.events.pop_front() {
            if let Some(WindowEvent::Resized(window)) = event.downcast_ref::<WindowEvent>() {
                self.context.window = *window;
            }
            let (area, layout) = self.layout();
            let mut passed: Option<Box<dyn DynEvent>> = None;
            event.pass_dyn(&mut self.context, &mut [ChildEvent {area: &area, event: &mut passed}]);
            if let Some(event) = passed {self.app.event(&mut self.context, &layout, event);}
        }
        let (area, layout) = self.layout();
        let bound = (area.offset.0, area.offset.1, area.size.0, area.size.1);
        self.app.draw(&layout, area.offset, bound)
    }

    fn layout(&mut self) -> (Area, LayoutTree) {
        let mut area = self.context.window.area();
        let range = self.app.size_range();
        let layout = self.app.layout(area.size, &range);
        area.size = (area.size.0.min(layout.0.0), area.size.1.min(layout.0.1));
        (area, layout)
    }
}
