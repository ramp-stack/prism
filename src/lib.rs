use std::path::{PathBuf, Path};
use std::fmt::Debug;
use std::any::Any;

pub use air::names::{Name, Id};
pub use air::contract::{Contract, Reactant, Substance, RequestBuilder, Error, Request};

use event::{Event, TickEvent};
use drawable::{Drawable, RequestTree, SizedTree};
use canvas::{Area, Item};

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
    fn me(&self) -> Name;

    ///TODO: remove
    fn builder(&self) -> &RequestBuilder;
    fn request(&mut self, request: Request);
    fn list(&self, c_id: Id) -> Vec<Id>;
    fn get(&self, c_id: Id, id: Id, path: PathBuf) -> Option<Substance>;

    fn start_camera(&mut self) -> Box<dyn Camera>;
    fn pick_photo(&mut self);

    fn get_safe_area(&self) -> (f32, f32, f32, f32);
    fn share_social(&mut self, data: String);

    fn set_clipboard(&mut self, data: String);
    fn get_clipboard(&self) -> Option<String>;

    fn trigger_haptic(&self);
}

pub struct Context(&'static mut dyn Handler, &'static mut Vec<Box<dyn Event>>);
impl Context {
    fn new(handler: &mut dyn Handler, events: &mut Vec<Box<dyn Event>>) -> Self {
        //This code is used so that during the event function triggered in the Instance I can pass around a &mut Context with out lifetime issues.
        unsafe { Context(
            std::mem::transmute::<&mut dyn Handler, &'static mut dyn Handler>(handler),
            std::mem::transmute::<&mut Vec<Box<dyn Event>>, &'static mut Vec<Box<dyn Event>>>(events)
        )}
    }

    pub fn me(&self) -> Name {self.0.me()}

    pub fn get<C: Contract, P: AsRef<Path>>(&self, iid: &Id, path: P) -> Option<Substance> {
        self.0.get(C::id(), *iid, path.as_ref().to_path_buf())
    }

    pub fn list<C: Contract>(&self) -> Vec<Id> {self.0.list(C::id())}

    pub fn create<C: Contract>(&mut self, contract: C) -> Result<Id, Error> {
        let (id, request) = self.0.builder().create(contract)?;
        self.0.request(request);
        Ok(id)
    }

    pub fn share<C: Contract>(&mut self, iid: Id, name: Name) -> Result<(), Error> {
        let request = self.0.builder().share::<C>(iid, name)?;
        self.0.request(request);
        Ok(())
    }

    pub fn send<P: AsRef<Path>, R: Reactant + 'static>(&mut self, id: Id, path: P, reactant: R) -> Result<Result<(), R::Error>, Error> {
        let request = self.0.builder().send(id, path, reactant)?;
        self.0.request(request);
        Ok(Ok(()))
    }

    pub fn emit<E: Event>(&mut self, event: E) {self.1.push(Box::new(event))}

    pub fn start_camera(&mut self) -> Box<dyn Camera> {self.0.start_camera()}
    pub fn pick_photo(&mut self) {self.0.pick_photo()}

    pub fn get_safe_area(&self) -> (f32, f32, f32, f32) {self.0.get_safe_area()}
    pub fn share_social(&mut self, data: String) {self.0.share_social(data)}

    pub fn set_clipboard(&mut self, data: String) {self.0.set_clipboard(data);}
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
        let mut events = Vec::new();
        let mut context = Context::new(handler, &mut events);
        let app = builder(&mut context);
        let size_request = app.request_size();
        let sized_app = app.build(screen, &size_request);

        Instance {
            app: Box::new(app),
            screen,
            request: size_request,
            size: sized_app,
            events
        }
    }

    pub fn resize(&mut self, screen: (f32, f32)) {
        self.screen = screen;
        self.size = self.app.build(self.screen, &self.request);
    }

    pub fn emit<E: Event>(&mut self, event: E) {self.events.push(Box::new(event));}

    pub fn draw(&mut self, handler: &mut dyn Handler) -> Vec<(Area, Item)> {
        let mut context = Context::new(handler, &mut self.events);
        self.app.event(&mut context, &self.size, Box::new(TickEvent));
        let events = self.events.drain(..).rev().collect::<Vec<_>>();
        let mut context = Context::new(handler, &mut self.events);
        for event in events {
            if let Some(event) = event
                .pass(&mut context, &[prism::layout::Area{offset: (0.0, 0.0), size: self.size.0}])
                .remove(0)
            {
                self.app.event(&mut context, &self.size, event);
            }
        }

        self.request = self.app.request_size();
        self.size = self.app.build(self.screen, &self.request);
        self.app.draw(&self.size, (0.0, 0.0), (0.0, 0.0, self.screen.0, self.screen.1))
    }
}
