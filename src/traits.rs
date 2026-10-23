use air::{Identity, Name, Response};
use dyn_clone::{DynClone, clone_trait_object};
pub use futures::future::BoxFuture;
use futures::future::LocalBoxFuture;

use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::io::{Read, Seek, Write};
use std::iter::FusedIterator;
use std::ops::{Bound, Deref};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::air::{ContractType, Id, Request, Value};
use crate::hardware::{
    Audio, Camera, Capture as CaptureData, Chunks as MediaChunks, Data, Format, Frame, Image as ImageData,
    Media as MediaData, Microphone, PickResult, PlaybackMode, Sound, Tracks, Video, Visual,
};
use crate::service::{Key, Service, ServiceInit};

pub type ContractUpdate = Update<Box<dyn DynPending>, Value>;

#[derive(Clone, Debug)]
pub enum Update<P, R> {Init, Pending(P), Confirmed(R)}

pub trait Air: Clone + Send + Sync + 'static {
    fn me(&self) -> Name;
    fn instances(&self, contract: ContractType) -> Box<dyn DynInstances>;
    fn start(&self, service: ServiceInit);
    fn shutdown(&self, key: Key);
}

pub trait Picker<T: Clone + 'static> {
    type Picked: Picked<T>;
    fn pick(&self) -> Self::Picked;
}

pub trait Hardware: Clone + 'static + Picker<Data> + Picker<ImageData> + Picker<MediaData>
    + Picker<Visual> + Picker<Video> + Picker<Audio> + Picker<PathBuf>
    + Picker<Vec<Data>> + Picker<Vec<ImageData>> + Picker<Vec<MediaData>>
    + Picker<Vec<Visual>> + Picker<Vec<Video>> + Picker<Vec<Audio>> + Picker<Vec<PathBuf>> {
    type Image: Image;
    type Media: Media;

    fn read_image(&self, reader: &mut dyn Read) -> Result<Self::Image, String>;
    fn image_from_rgba(&self, rgba: Arc<image::RgbaImage>) -> Self::Image;

    fn media(&self) -> Self::Media;
    fn media_from_sound(&self, sound: Sound) -> Result<Self::Media, String>;
    fn media_from_image(&self, image: ImageData, duration: Duration) -> Result<Self::Media, String>;
    fn read_media(&self, reader: &mut dyn Reader) -> Result<Self::Media, String>;
    fn open_keyboard(&self) -> crate::hardware::Keyboard;
    fn start_camera(&self) -> Result<Camera, String>;
    fn start_microphone(&self) -> Result<Microphone, String>;
    fn start_capture(&self) -> Result<CaptureData, String>;
    fn share(&self, data: &Data);
    fn set_clipboard(&self, data: &Data);
    fn get_clipboard(&self) -> Option<Data>;
    fn set_cache(&self, key: &str, data: &Data) -> Result<(), String>;
    fn get_cache(&self, key: &str) -> Result<Option<Data>, String>;
    fn trigger_haptic(&self);
}

pub trait Picked<T: Clone + 'static>: 'static {
    fn get(&mut self) -> impl Future<Output = PickResult<T>>;
}

pub trait Keyboard: DynClone + 'static {}
clone_trait_object!(Keyboard);

pub trait Capture: DynClone + 'static {
    fn error(&self) -> Option<String>;
}
clone_trait_object!(Capture);

pub trait Image: downcast_rs::DowncastSync + DynClone + Debug + PartialEq<ImageData>
    + Deref<Target = Arc<image::RgbaImage>> + Send + Sync + 'static {
    fn dimensions(&self) -> (u32, u32);
}
clone_trait_object!(Image);
downcast_rs::impl_downcast!(sync Image);

pub trait Media: downcast_rs::DowncastSync + DynClone + Debug + Send + Sync + 'static {
    fn player(&self) -> Result<Box<dyn Player>, String>;
    fn copy(&self, range: (Bound<Duration>, Bound<Duration>), tracks: Tracks) -> Result<Box<dyn Media>, String>;
    fn replace(&self, range: (Bound<Duration>, Bound<Duration>), replacement: &MediaData, tracks: Tracks) -> Result<(), String>;
    fn write(&self, writer: &mut dyn Writer, format: Format) -> Result<(), String>;
    fn duration(&self) -> Duration;
    fn dimensions(&self) -> Option<(u32, u32)>;
    fn thumbnail(&self) -> Option<ImageData>;
    fn append(&self, other: &MediaData);
    fn chunks(&self, duration: Duration) -> Result<MediaChunks, String>;
}
clone_trait_object!(Media);
downcast_rs::impl_downcast!(sync Media);

pub trait Player: DynClone + Debug + Send + Sync + 'static {
    fn set_mode(&self, mode: PlaybackMode);
    fn mode(&self) -> PlaybackMode;
    fn start(&self);
    fn stop(&self);
    fn set_speed(&self, speed: f64);
    fn speed(&self) -> f64;
    fn set_volume(&self, volume: f64);
    fn volume(&self) -> f64;
    fn seek(&self, position: Duration);
    fn position(&self) -> Duration;
    fn duration(&self) -> Duration;
    fn is_playing(&self) -> bool;
    fn error(&self) -> Option<String>;
    fn frame(&self) -> Frame;
    fn next(&mut self) -> BoxFuture<'_, Frame>;
}
clone_trait_object!(Player);

pub trait DynRef: Debug {
    fn value(&self) -> &(dyn Any + Send + Sync);
}

pub trait DynInstances: DynClone + Debug + Send + Sync {
    fn create(&self, init: Request) -> Box<dyn DynInstance>;
    fn list(&self) -> HashMap<Id, Box<dyn DynInstance>>;
    fn try_next(&mut self) -> Option<Box<dyn DynInstance>>;
    fn next(&mut self) -> BoxFuture<'_, Box<dyn DynInstance>>;
}
clone_trait_object!(DynInstances);

pub trait DynInstance: DynClone + Debug + Send + Sync {
    fn id(&self) -> Id;
    fn notify(&self, enabled: bool);
    fn send(&mut self, message: Request) -> Box<dyn DynPending>;
    fn share(&self, name: Name);
    fn pending(&self) -> Box<dyn DynRef + '_>;
    fn confirmed(&self) -> Option<Box<dyn DynRef + '_>>;
    fn try_next(&mut self) -> Option<ContractUpdate>;
    fn next(&mut self) -> BoxFuture<'_, ContractUpdate>;
}
clone_trait_object!(DynInstance);

pub trait DynPending: DynClone + Debug + Send + Sync {
    fn pending(&self) -> Box<dyn DynRef + '_>;
    fn try_confirmed(&self) -> Option<Value>;
    fn try_next(&mut self) -> Option<bool>;
    fn next(&mut self) -> BoxFuture<'_, bool>;
}
clone_trait_object!(DynPending);

/// Protocol adapter supplied by each contract's typed facade; execution stays in the backend.
pub trait DynContract: DynClone + Debug + erased_serde::Serialize + Send + Sync {
    fn id(&self) -> Id;
    fn location(&self) -> air::Location;
    fn subscription(&self) -> Option<(air::PublicKey, Name)>;
    fn pending(&self) -> Option<&(dyn Any + Send + Sync)>;
    fn confirmed(&self) -> Option<&(dyn Any + Send + Sync)>;
    fn send(&mut self, message: Request) -> Value;
    fn start(&mut self) -> air::Request;
    fn request(&mut self) -> Option<air::Request>;
    fn response<'a>(&'a mut self, resolver: &'a dyn Resolver, response: Response) -> BoxFuture<'a, air::Output<Value>>;
}
clone_trait_object!(DynContract);
erased_serde::serialize_trait_object!(DynContract);

pub trait Resolver: Send + Sync {
    fn resolve(&self, name: Name, timestamp: Option<u64>) -> BoxFuture<'_, Arc<Identity>>;
}

pub trait Reader: Read + Seek {}
impl<R: Read + Seek> Reader for R {}

pub trait Writer: Write + Seek {}
impl<W: Write + Seek> Writer for W {}

pub trait Chunks: DoubleEndedIterator<Item = MediaData> + ExactSizeIterator + FusedIterator
    + DynClone + Debug + Send + 'static {}
clone_trait_object!(Chunks);
impl<I: DoubleEndedIterator<Item = MediaData> + ExactSizeIterator + FusedIterator
    + Clone + Debug + Send + 'static> Chunks for I {}

pub trait DynService: Send {
    fn run(&mut self) -> BoxFuture<'_, ()>;
}
impl<S: Service> DynService for S {
    fn run(&mut self) -> BoxFuture<'_, ()> {
        Box::pin(async move {
            let predicate = self.predicate().await;
            self.handle(predicate).await;
        })
    }
}

pub trait DynAir: DynClone + Send + Sync {
    fn me(&self) -> Name;
    fn instances(&self, contract: ContractType) -> Box<dyn DynInstances>;
    fn start(&self, service: ServiceInit);
    fn shutdown(&self, key: Key);
}
clone_trait_object!(DynAir);
impl<A: Air> DynAir for A {
    fn me(&self) -> Name {Air::me(self)}
    fn instances(&self, contract: ContractType) -> Box<dyn DynInstances> {Air::instances(self, contract)}
    fn start(&self, service: ServiceInit) {Air::start(self, service)}
    fn shutdown(&self, key: Key) {Air::shutdown(self, key)}
}

pub trait DynPicker<T: Clone + 'static> {
    fn pick(&self) -> Box<dyn DynPicked<T>>;
}
impl<T: Clone + 'static, H: Picker<T>> DynPicker<T> for H {
    fn pick(&self) -> Box<dyn DynPicked<T>> {Box::new(Picker::pick(self))}
}

pub trait DynPicked<T: Clone + 'static> {
    fn get(&mut self) -> LocalBoxFuture<'_, PickResult<T>>;
}

impl<T: Clone + 'static, P: Picked<T>> DynPicked<T> for P {
    fn get(&mut self) -> LocalBoxFuture<'_, PickResult<T>> {Box::pin(Picked::get(self))}
}

pub trait DynHardware: DynClone + DynPicker<Data> + DynPicker<ImageData> + DynPicker<MediaData>
    + DynPicker<Visual> + DynPicker<Video> + DynPicker<Audio> + DynPicker<PathBuf>
    + DynPicker<Vec<Data>> + DynPicker<Vec<ImageData>> + DynPicker<Vec<MediaData>>
    + DynPicker<Vec<Visual>> + DynPicker<Vec<Video>> + DynPicker<Vec<Audio>> + DynPicker<Vec<PathBuf>> {
    fn read_image(&self, reader: &mut dyn Read) -> Result<Box<dyn Image>, String>;
    fn image_from_rgba(&self, rgba: Arc<image::RgbaImage>) -> Box<dyn Image>;
    fn media(&self) -> Box<dyn Media>;
    fn media_from_sound(&self, sound: Sound) -> Result<Box<dyn Media>, String>;
    fn media_from_image(&self, image: ImageData, duration: Duration) -> Result<Box<dyn Media>, String>;
    fn read_media(&self, reader: &mut dyn Reader) -> Result<Box<dyn Media>, String>;
    fn open_keyboard(&self) -> crate::hardware::Keyboard;
    fn start_camera(&self) -> Result<Camera, String>;
    fn start_microphone(&self) -> Result<Microphone, String>;
    fn start_capture(&self) -> Result<CaptureData, String>;
    fn share(&self, data: &Data);
    fn set_clipboard(&self, data: &Data);
    fn get_clipboard(&self) -> Option<Data>;
    fn set_cache(&self, key: &str, data: &Data) -> Result<(), String>;
    fn get_cache(&self, key: &str) -> Result<Option<Data>, String>;
    fn trigger_haptic(&self);
}
clone_trait_object!(DynHardware);

impl<H: Hardware> DynHardware for H {
    fn read_image(&self, reader: &mut dyn Read) -> Result<Box<dyn Image>, String> {
        Hardware::read_image(self, reader).map(|image| Box::new(image) as _)
    }
    fn image_from_rgba(&self, rgba: Arc<image::RgbaImage>) -> Box<dyn Image> {
        Box::new(Hardware::image_from_rgba(self, rgba))
    }
    fn media(&self) -> Box<dyn Media> {Box::new(Hardware::media(self))}
    fn media_from_sound(&self, sound: Sound) -> Result<Box<dyn Media>, String> {
        Hardware::media_from_sound(self, sound).map(|media| Box::new(media) as _)
    }
    fn media_from_image(&self, image: ImageData, duration: Duration) -> Result<Box<dyn Media>, String> {
        Hardware::media_from_image(self, image, duration).map(|media| Box::new(media) as _)
    }
    fn read_media(&self, reader: &mut dyn Reader) -> Result<Box<dyn Media>, String> {
        Hardware::read_media(self, reader).map(|media| Box::new(media) as _)
    }
    fn open_keyboard(&self) -> crate::hardware::Keyboard {Hardware::open_keyboard(self)}
    fn start_camera(&self) -> Result<Camera, String> {Hardware::start_camera(self)}
    fn start_microphone(&self) -> Result<Microphone, String> {Hardware::start_microphone(self)}
    fn start_capture(&self) -> Result<CaptureData, String> {Hardware::start_capture(self)}
    fn share(&self, data: &Data) {Hardware::share(self, data)}
    fn set_clipboard(&self, data: &Data) {Hardware::set_clipboard(self, data)}
    fn get_clipboard(&self) -> Option<Data> {Hardware::get_clipboard(self)}
    fn set_cache(&self, key: &str, data: &Data) -> Result<(), String> {Hardware::set_cache(self, key, data)}
    fn get_cache(&self, key: &str) -> Result<Option<Data>, String> {Hardware::get_cache(self, key)}
    fn trigger_haptic(&self) {Hardware::trigger_haptic(self)}
}
