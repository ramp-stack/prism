use std::path::{PathBuf, Path};

pub use air::names::{Name, Id};
pub use air::contract::{Contract, Reactant, Substance, RequestBuilder, Error, Request};

use crate::event::Event;

pub mod event;
pub mod layout;
pub mod drawable;
pub mod display;
pub mod emitters;

pub use wgpu_canvas as canvas;

extern crate self as prism;

pub trait Handler {
    fn me(&mut self) -> Name;

    fn builder(&self) -> &RequestBuilder;
    fn request(&mut self, request: Request);
    fn get(&mut self, c_id: Id, id: Id, path: PathBuf) -> Option<Substance>;

    fn start_camera(&mut self);
    fn stop_camera(&mut self);
    fn pick_photo(&mut self);

    fn get_safe_area(&mut self) -> (f32, f32, f32, f32);
    fn share_social(&mut self, data: String);

    fn set_clipboard(&mut self, data: String);
    fn get_clipboard(&mut self) -> String;

    fn trigger_haptic(&mut self);
}

pub struct Context(Box<dyn Handler>, pub Vec<Box<dyn Event>>);
impl Context {
    pub fn new<H: Handler + 'static>(handler: H) -> Self {Context(Box::new(handler), Vec::new())}

    pub fn me(&mut self) -> Name {self.0.me()}

    pub fn get<C: Contract, P: AsRef<Path>>(&mut self, iid: &Id, path: P) -> Option<Substance> {
        self.0.get(C::id(), *iid, path.as_ref().to_path_buf())
    }

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

    pub fn start_camera(&mut self) {self.0.start_camera()}
    pub fn stop_camera(&mut self) {self.0.stop_camera()}
    pub fn pick_photo(&mut self) {self.0.pick_photo()}
    pub fn get_safe_area(&mut self) -> (f32, f32, f32, f32) {self.0.get_safe_area()}
    pub fn share_social(&mut self, data: String) {self.0.share_social(data)}

    pub fn set_clipboard(&mut self, data: String) {self.0.set_clipboard(data);}
    pub fn get_clipboard(&mut self) -> String {self.0.get_clipboard()}

    pub fn trigger_haptic(&mut self) {self.0.trigger_haptic()}
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

pub struct Assets(pub Dir<'static>);
impl Assets {
    pub fn load_file(&self, file: &str) -> Option<Vec<u8>> {
        self.0.entries().iter().find_map(|e| match e {
            DirEntry::File(f) => (f.path().to_str().unwrap().to_lowercase() == file.to_lowercase()).then_some(f.contents().to_vec()),
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

    pub fn load_image(&self, file: &str) -> Option<RgbaImage> {
        let bytes = Assets::load_file(self, file).expect("No file");
        Some(image::load_from_memory(&bytes).expect("Unsupported or corrupt image").into_rgba8())
    }
}
