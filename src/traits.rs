use air::{Identity, Name, Response};
use dyn_clone::{DynClone, clone_trait_object};
pub use futures::future::BoxFuture;
use futures::future::LocalBoxFuture;

use std::any::Any;
use std::collections::HashMap;
use std::fmt::Debug;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::air::{ContractType, Id, Request, Value};
use crate::hardware::{
    Audio, Camera, Capture as CaptureData, Data, Frame, Image as ImageData, Media as MediaData,
    Microphone, PickResult, PlaybackMode, Player as PlayerData, Video, Visual,
};
use crate::service::{Key, Service, ServiceInit};

pub type ContractUpdate = Update<Box<dyn DynPending>, Value>;

#[derive(Clone, Debug)]
pub enum Update<P, R> {Init, Pending(P), Confirmed(R)}

pub trait Keyboard: DynClone + 'static {
    fn show(&self, dismiss_padding: f32);
    fn hide(&self);
    fn visible(&self) -> bool;
}
clone_trait_object!(Keyboard);

pub trait Capture<T>: DynClone + 'static {
    fn next(&mut self) -> LocalBoxFuture<'_, Option<T>>;
    fn error(&self) -> Option<String>;
}
clone_trait_object!(<T> Capture<T>);

pub trait Player: DynClone + Debug + Send + Sync + 'static {
    fn has_video(&self) -> bool;
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
    fn duration(&self) -> Option<Duration>;
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

pub trait DynPicker<T: Clone + 'static> {
    fn pick(&self) -> Box<dyn DynPicked<T>>;
}

pub trait DynPicked<T: Clone + 'static> {
    fn get(&mut self) -> LocalBoxFuture<'_, PickResult<T>>;
}

pub trait DynHardware: DynClone + DynPicker<Data> + DynPicker<ImageData> + DynPicker<MediaData>
    + DynPicker<Visual> + DynPicker<Video> + DynPicker<Audio> + DynPicker<PathBuf>
    + DynPicker<Vec<Data>> + DynPicker<Vec<ImageData>> + DynPicker<Vec<MediaData>>
    + DynPicker<Vec<Visual>> + DynPicker<Vec<Video>> + DynPicker<Vec<Audio>> + DynPicker<Vec<PathBuf>> {
    fn keyboard(&self) -> crate::hardware::Keyboard;
    fn player(&self, player: media::Player) -> Result<PlayerData, String>;
    fn start_camera(&self) -> Result<Camera, String>;
    fn start_microphone(&self) -> Result<Microphone, String>;
    fn start_capture(&self) -> Result<CaptureData, String>;
    fn share(&self, data: &Data);
    fn set_clipboard(&self, data: &Data);
    fn get_clipboard(&self) -> Option<Data>;
    fn set_cache(&self, key: &str, data: &Data) -> Result<(), String>;
    fn get_cache(&self, key: &str) -> Option<Data>;
    fn trigger_haptic(&self);
}
clone_trait_object!(DynHardware);

pub trait Air: Clone + Send + Sync + 'static {
    fn me(&self) -> Name;
    fn instances(&self, contract: ContractType) -> Box<dyn DynInstances>;
    fn start(&self, service: ServiceInit);
    fn shutdown(&self, key: Key);
}
impl<A: Air> DynAir for A {
    fn me(&self) -> Name {Air::me(self)}
    fn instances(&self, contract: ContractType) -> Box<dyn DynInstances> {Air::instances(self, contract)}
    fn start(&self, service: ServiceInit) {Air::start(self, service)}
    fn shutdown(&self, key: Key) {Air::shutdown(self, key)}
}

pub trait Picker<T: Clone + 'static> {
    type Picked: Picked<T>;
    fn pick(&self) -> Self::Picked;
}
impl<T: Clone + 'static, H: Picker<T>> DynPicker<T> for H {
    fn pick(&self) -> Box<dyn DynPicked<T>> {Box::new(Picker::pick(self))}
}

pub trait Picked<T: Clone + 'static>: 'static {
    fn get(&mut self) -> impl Future<Output = PickResult<T>>;
}
impl<T: Clone + 'static, P: Picked<T>> DynPicked<T> for P {
    fn get(&mut self) -> LocalBoxFuture<'_, PickResult<T>> {Box::pin(Picked::get(self))}
}

pub trait Hardware: Clone + 'static + Picker<Data> + Picker<ImageData> + Picker<MediaData>
    + Picker<Visual> + Picker<Video> + Picker<Audio> + Picker<PathBuf>
    + Picker<Vec<Data>> + Picker<Vec<ImageData>> + Picker<Vec<MediaData>>
    + Picker<Vec<Visual>> + Picker<Vec<Video>> + Picker<Vec<Audio>> + Picker<Vec<PathBuf>> {
    fn keyboard(&self) -> crate::hardware::Keyboard;
    fn player(&self, player: media::Player) -> Result<PlayerData, String>;
    fn start_camera(&self) -> Result<Camera, String>;
    fn start_microphone(&self) -> Result<Microphone, String>;
    fn start_capture(&self) -> Result<CaptureData, String>;
    fn share(&self, data: &Data);
    fn set_clipboard(&self, data: &Data);
    fn get_clipboard(&self) -> Option<Data>;
    fn set_cache(&self, key: &str, data: &Data) -> Result<(), String>;
    fn get_cache(&self, key: &str) -> Option<Data>;
    fn trigger_haptic(&self);
}
impl<H: Hardware> DynHardware for H {
    fn keyboard(&self) -> crate::hardware::Keyboard {Hardware::keyboard(self)}
    fn player(&self, player: media::Player) -> Result<PlayerData, String> {Hardware::player(self, player)}
    fn start_camera(&self) -> Result<Camera, String> {Hardware::start_camera(self)}
    fn start_microphone(&self) -> Result<Microphone, String> {Hardware::start_microphone(self)}
    fn start_capture(&self) -> Result<CaptureData, String> {Hardware::start_capture(self)}
    fn share(&self, data: &Data) {Hardware::share(self, data)}
    fn set_clipboard(&self, data: &Data) {Hardware::set_clipboard(self, data)}
    fn get_clipboard(&self) -> Option<Data> {Hardware::get_clipboard(self)}
    fn set_cache(&self, key: &str, data: &Data) -> Result<(), String> {Hardware::set_cache(self, key, data)}
    fn get_cache(&self, key: &str) -> Option<Data> {Hardware::get_cache(self, key)}
    fn trigger_haptic(&self) {Hardware::trigger_haptic(self)}
}
