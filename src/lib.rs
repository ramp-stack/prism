use std::fmt::Debug;
use std::any::Any;

pub use air::{Name, Id};
pub use air::{Contract, Reactant};

use event::{Event, TickEvent};
use drawable::{Drawable, RequestTree, SizedTree};
use canvas::Instruction;

pub mod event;
pub mod layout;
pub mod drawable;
pub mod display;
pub mod emitters;

pub use wgpu_canvas as canvas;

extern crate self as prism;

pub const IS_MOBILE: bool = cfg!(any(target_os = "ios", target_os = "android"));
pub const IS_WEB: bool = cfg!(target_arch = "wasm32");

///A handler trait for a Camera, It is assumed that CameraFrame events will be emmited for as long
///as one of these Handlers exists.
pub trait Camera: Any + Debug {fn clone_camera(&self) -> Box<dyn Camera>;}
impl<C: Any + Debug + Clone> Camera for C {
    fn clone_camera(&self) -> Box<dyn Camera> {Box::new(self.clone())}
}
impl Clone for Box<dyn Camera> {fn clone(&self) -> Self {(**self).clone_camera()}}

pub trait Handler {
    fn air(&self) -> &air::Context;
    fn start_camera(&self) -> Box<dyn Camera>;
    fn pick_photo(&self);

    fn get_safe_area(&self) -> (f32, f32, f32, f32);
    fn share_social(&self, data: String);

    fn set_clipboard(&self, data: String);
    fn get_clipboard(&self) -> Option<String>;

    fn trigger_haptic(&self);
}

pub struct Context(&'static mut dyn Handler, Vec<Box<dyn Event>>);
impl Context {
    fn new(handler: &mut dyn Handler) -> Self {
        unsafe { Context(
            std::mem::transmute::<&mut dyn Handler, &'static mut dyn Handler>(handler),
            Vec::new()
        )}
    }

    pub fn me(&self) -> Name {self.0.air().me()}
    pub fn create<C: Contract>(&self, init: C::Init) -> air::Instance<C> {self.0.air().create::<C>(init)}
    pub fn list<C: Contract>(&self) -> Vec<air::Instance<C>> {self.0.air().list::<C>()}
    pub fn register<C: Contract>(&self) {self.0.air().register::<C>()}

    pub fn emit<E: Event>(&mut self, event: E) {self.1.push(Box::new(event))}

    pub fn start_camera(&self) -> Box<dyn Camera> {self.0.start_camera()}
    pub fn pick_photo(&self) {self.0.pick_photo()}

    pub fn get_safe_area(&self) -> (f32, f32, f32, f32) {self.0.get_safe_area()}
    pub fn share_social(&self, data: String) {self.0.share_social(data)}

    pub fn set_clipboard(&self, data: String) {self.0.set_clipboard(data);}
    pub fn get_clipboard(&self) -> Option<String> {self.0.get_clipboard()}

    pub fn trigger_haptic(&self) {self.0.trigger_haptic()}
}

pub struct Instance {
    app: Box<dyn Drawable>,
    screen: (f32, f32),
    request: RequestTree,
    size: SizedTree,
    events: Vec<Box<dyn Event>>
}

impl Instance {
    pub fn new<D: Drawable>(builder: impl FnOnce(&mut Context) -> D, handler: &mut dyn Handler, screen: (f32, f32)) -> Self {
        let mut context = Context::new(handler);
        let app = builder(&mut context);
        let size_request = app.request_size();
        let sized_app = app.build(screen, &size_request);

        Instance {
            app: Box::new(app),
            screen,
            request: size_request,
            size: sized_app,
            events: context.1
        }
    }

    pub fn resize(&mut self, screen: (f32, f32)) {
        self.screen = screen;
        self.size = self.app.build(self.screen, &self.request);
    }

    pub fn emit<E: Event>(&mut self, event: E) {self.events.push(Box::new(event));}

    pub fn draw(&mut self, handler: &mut dyn Handler) -> Vec<Instruction> {
        let mut context = Context::new(handler);
        self.app.event(&mut context, &self.size, Box::new(TickEvent));
        let mut events = self.events.drain(..).rev().collect::<Vec<_>>();
        events.extend(context.1);
        let mut context = Context::new(handler);
        for event in events {
            if let Some(event) = event
                .pass(&mut context, &[prism::layout::Area{offset: (0.0, 0.0), size: self.size.0}])
                .remove(0)
            {
                self.app.event(&mut context, &self.size, event);
            }
        }
        self.events = context.1;
        self.request = self.app.request_size();
        self.size = self.app.build(self.screen, &self.request);
        self.app.draw(&self.size, (0.0, 0.0), (0.0, 0.0, self.screen.0, self.screen.1))
    }
}


type OnTick<C> = Box<dyn FnMut(&mut Context, &C)>;
pub struct ContractListener<C: Contract> {
    contract: C,
    on_tick: OnTick<C>,
    update: Box<dyn FnMut() -> C>,
}

impl<C: Contract> ContractListener<C> {
    pub fn new(contract: C, on_tick: impl FnMut(&mut Context, &C) + 'static, update: impl FnMut() -> C + 'static) -> Self {
        ContractListener {contract, on_tick: Box::new(on_tick), update: Box::new(update)}
    }

    pub fn update(&mut self) {
        self.contract = (self.update)();
    }

    pub fn tick(&mut self, ctx: &mut Context) {
        (self.on_tick)(ctx, &self.contract)
    }
}
