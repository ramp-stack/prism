use std::sync::mpsc::Sender;
use crate::event::Event;

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
    Service(String, String)
}

pub struct FrameSettings {

}

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

/// There are three context actions which should be converted into serialized actions
/// 1. Manipulate State where state is the only input and output
/// 2. Send a request to the OS a Hardware or Service request
/// 3. Send an event to be triggered
pub struct Context {
    pub state: State,
    pub sender: Sender<Request>,
}

impl Context {
    pub fn send(&mut self, request: Request) {
        self.sender.send(request).expect("Issue with channel");
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
