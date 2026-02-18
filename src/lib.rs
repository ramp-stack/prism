use std::sync::mpsc::Sender;
use std::sync::mpsc::{Receiver, channel};
use std::hash::{Hash, Hasher, DefaultHasher};
use std::marker::PhantomData;
use std::collections::VecDeque;
use std::fmt::Debug;
use std::sync::{Arc, Mutex};

use crate::layout::Stack;
use crate::event::{OnEvent, Event, TickEvent};
use crate::drawable::{Drawable, SizedTree, Component};

pub mod event;
pub mod layout;
pub mod drawable;
pub mod display;
pub mod emitters;

pub use wgpu_canvas as canvas;

extern crate self as prism;

pub enum Request {
    Event(Box<dyn Event>),
    Hardware(Hardware),
    Service(String, String),
    Listener(Box<dyn Fn(&mut State)>),
}

impl Request {
    pub fn event(e: impl Event + 'static) -> Self {Request::Event(Box::new(e))}
}

#[derive(Debug)]
pub struct FrameSettings {}

#[derive(Debug)]
pub enum Hardware {
    CameraStart,
    CameraFrame(FrameSettings),
    CameraStop,
    PhotoPicker,
    SetClipboard(String),
    GetClipboard,
    SetCloud(String, String),
    GetCloud(String),
    Share(String),
    Haptic,
}

anyanymap::Map!(State: );
//implement btree from typeid to serde_json value. look things up by typeid then desereliazes

pub struct StoredHash<T: Hash>(u64, PhantomData::<fn() -> T>);

impl<T: Hash> Default for StoredHash<T> {
    fn default() -> Self {
        StoredHash(u64::default(), PhantomData)
    }
}

type ListenerFn = Box<dyn Fn(&mut State)>;

pub struct Instance {
    receiver: Receiver<Request>,
    listeners: Vec<ListenerFn>,
    pub events: VecDeque<Box<dyn prism::event::Event>>,
}

impl Instance {
    pub fn new(receiver: Receiver<Request>) -> Self {
        Instance { receiver, listeners: vec![], events: VecDeque::new() }
    }
    pub fn add_listener(&mut self, listener: ListenerFn) {
        self.listeners.push(listener);
    }

    pub fn tick(&mut self, ctx: &mut Context) {
        self.listeners.iter().for_each(|l| (l)(&mut ctx.state));
    }

    pub fn handle_requests(&mut self) {
        while let Ok(request) = self.receiver.try_recv() {
            match request {
                prism::Request::Listener(listener) => self.add_listener(listener),
                prism::Request::Event(event) => self.events.push_back(event),
                prism::Request::Hardware(hardware) => println!("Attempting to start {hardware:?}"),
                    // x => println!("Attempting to start {x:?}")
                    //CameraStart,
                    //CameraFrame(FrameSettings),
                    //CameraStop,
                    //PhotoPicker,
                    //SetClipboard(String),
                    //GetClipboard,
                    //SetCloud(String, String),
                    //GetCloud(String),
                    //Share(String),
                    //Haptic,
                // },
                _ => {}
            }
        }
    }
}

/// There are three context actions which should be converted into serialized actions
/// 1. Manipulate State where state is the only input and output
/// 2. Send a request to the OS a Hardware or Service request
/// 3. Send an event to be triggered
pub struct Context {
    state: State, // TODO: remove state from context and replace with sql
    pub sender: Sender<Request>,
}

impl Context {
    pub fn new() -> (Self, Receiver<Request>) {
        let (sender, receiver) = channel();
        (Context{state: State::default(), sender}, receiver)
    }

    pub fn send(&mut self, request: Request) {
        self.sender.send(request).expect("Issue with channel");
    }

    pub fn register_listener<T: Hash + Clone + 'static>(&mut self) -> Receiver<T> {
        println!("Registering listener");
        let (sender, receiver) = channel();
        let _ = self.sender.send(Request::Listener(Box::new(move |state: &mut State| {
            let previous_hash = state.get_mut::<StoredHash<T>>().as_mut().map(|s| s.0).unwrap_or_default();
            if let Some(t) = state.get::<T>() {
                let mut hasher = DefaultHasher::new();
                t.hash(&mut hasher);
                let new_hash = hasher.finish();
                if previous_hash != new_hash {
                    let _ = sender.send(t.clone());
                    state.get_mut_or_default::<StoredHash<T>>().0 = new_hash;
                }
            }
        })));
        receiver
    }
}

type UpdatedOn<D, T> = Arc<dyn Fn(&mut Context, &mut D, T) + Send + Sync + 'static>;

#[derive(Component, Clone)]
pub struct Listener<D: Drawable + Clone, T: Hash + Clone + Debug + 'static> {
    layout: Stack,
    inner: D,
    #[skip] receiver: Arc<Mutex<Receiver<T>>>,
    #[skip] updated_on: UpdatedOn<D, T>,
}

impl<D: Drawable + Clone, T: Hash + Clone + Debug + 'static> Debug for Listener<D, T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Listener").field("inner", &self.inner).finish()
    }
}

impl<D: Drawable + Clone, T: Hash + Clone + Debug + 'static> OnEvent for Listener<D, T> {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if event.downcast_ref::<TickEvent>().is_some() 
        && let Ok(receiver) = self.receiver.lock() 
        && let Ok(val) = receiver.try_recv() {
            (self.updated_on)(ctx, &mut self.inner, val)
        }
        vec![event]
    }
}

impl<D: Drawable + Clone, T: Hash + Debug + Clone + 'static> Listener<D, T> {
    pub fn new(ctx: &mut Context, inner: D, updated_on: impl Fn(&mut Context, &mut D, T) + Send + Sync + 'static) -> Self {
        Listener{
            layout: Stack::default(),
            receiver: Arc::new(Mutex::new(ctx.register_listener::<T>())),
            inner,
            updated_on: Arc::new(updated_on)
        }
    }
}


/// `true` if the target platform is iOS or Android, otherwise `false`.
#[cfg(any(target_os = "ios", target_os = "android"))]
pub const IS_MOBILE: bool = true;
#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub const IS_MOBILE: bool = false;

/// `true` if the target architecture is WebAssembly (`wasm32`), otherwise `false`.
#[cfg(target_arch = "wasm32")]
pub const IS_WEB: bool = true;
#[cfg(not(target_arch = "wasm32"))]
pub const IS_WEB: bool = false;

use image::RgbaImage;
use include_dir::{DirEntry, Dir};

pub struct Assets;
impl Assets {
    pub fn load_file(dir: &Dir, file: &str) -> Option<Vec<u8>> {
        dir.entries().iter().find_map(|e| match e {
            DirEntry::File(f) => (f.path().to_str().unwrap() == file).then_some(f.contents().to_vec()),
            _ => None,
        })
    }

    pub fn load_svg(svg: &[u8]) -> RgbaImage {
        let svg = std::str::from_utf8(svg).unwrap();
        let svg = nsvg::parse_str(svg, nsvg::Units::Pixel, 96.0).unwrap();
        let rgba = svg.rasterize(8.0).unwrap();
        let size = rgba.dimensions();
        RgbaImage::from_raw(size.0, size.1, rgba.into_raw()).unwrap()
    }
}