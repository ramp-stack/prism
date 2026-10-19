use air::{Contract, Id, Name};
use dyn_clone::{DynClone, clone_trait_object};
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use crate::hardware::{Data, Image, Media, PickResult, Video as VideoData};

#[derive(Clone, Debug)]
pub enum Update<P, R> {Init, Pending(P), Confirmed(R)}

pub trait Air {
    type Instance<C: Contract>: Instance<C>;
    type Instances<C: Contract>: Instances<C, Instance = Self::Instance<C>>;

    fn instances<C: Contract>(&self) -> Self::Instances<C>;
    fn me(&self) -> Name;
}

pub trait Instance<C: Contract>: Clone + Debug + Send + Sync + 'static {
    type Pending: Pending<C::Result>;
    type Ref<'a>: Ref<'a, C>;

    fn send(&mut self, msg: C::Message) -> Self::Pending;
    fn share(&self, name: Name);
    fn pending(&self) -> Self::Ref<'_>;
    fn confirmed(&self) -> Option<Self::Ref<'_>>;
    fn try_next(&mut self) -> Option<Update<Self::Pending, C::Result>>;
    fn next(&mut self) -> impl std::future::Future<Output = Update<Self::Pending, C::Result>> + Send;
}

pub trait Instances<C: Contract>: Clone + Debug + Send + Sync + 'static {
    type Instance: Instance<C>;

    fn create(&self, init: C::Init) -> Self::Instance;
    fn list(&self) -> HashMap<Id, Self::Instance>;
    fn try_next(&mut self) -> Option<Self::Instance>;
    fn next(&mut self) -> impl std::future::Future<Output = Self::Instance> + Send;
}

pub trait Pending<R: Debug + Clone + Send + Sync + 'static>: Debug + Clone + Send + 'static {
    type Ref<'a>: Ref<'a, R>;

    fn pending(&self) -> Self::Ref<'_>;
    fn try_confirmed(&self) -> Option<R>;
    fn confirmed(self) -> impl std::future::Future<Output = R> + Send;
    fn try_next(&mut self) -> Option<bool>;
    fn next(&mut self) -> impl std::future::Future<Output = bool> + Send;
}

pub trait Picker<T: Clone + 'static> {
    type Picked: Picked<T>;
    fn pick(&self) -> Self::Picked;
}

pub trait Hardware: Clone + 'static + Picker<Data> + Picker<Image> + Picker<VideoData>
    + Picker<Media> + Picker<PathBuf> + Picker<Vec<Data>> + Picker<Vec<Image>>
    + Picker<Vec<VideoData>> + Picker<Vec<Media>> + Picker<Vec<PathBuf>> {
    type Camera: Camera;
    type Video: Video;

    fn start_camera(&self) -> Self::Camera;
    fn video(&self, bytes: Vec<u8>) -> Option<Self::Video>;
    fn share(&self, data: &Data);
    fn set_clipboard(&self, data: &Data);
    fn get_clipboard(&self) -> Option<Data>;
    fn set_cache(&self, key: &str, data: &Data);
    fn get_cache(&self, key: &str) -> Option<Data>;
    fn trigger_haptic(&self);
}

pub trait Picked<T: Clone + 'static>: 'static {
    fn get(self) -> impl std::future::Future<Output = PickResult<T>>;
    fn try_get(&mut self) -> Option<PickResult<T>>;
}

pub trait Camera: DynClone + 'static {
    fn frame(&self) -> Image;
}
clone_trait_object!(Camera);

pub trait Video: DynClone + Debug + Send + Sync + 'static {
    fn bytes(&self) -> &Arc<Vec<u8>>;
    fn duration(&self) -> Option<Duration>;
    fn dimensions(&self) -> (u32, u32);
    fn thumbnail(&self) -> Image;
    fn format(&self) -> &str;
    fn codec(&self) -> &str;
    fn player(&self) -> Result<Box<dyn Player>, String>;
}
clone_trait_object!(Video);

pub trait Player: DynClone + Debug + Send + Sync + 'static {
    fn start(&self);
    fn stop(&self);
    fn set_speed(&self, speed: f64);
    fn speed(&self) -> f64;
    fn seek(&self, position: Duration);
    fn position(&self) -> Duration;
    fn duration(&self) -> Option<Duration>;
    fn is_playing(&self) -> bool;
    fn frame(&self) -> Image;
}
clone_trait_object!(Player);

pub trait Ref<'a, T>: Debug + Deref<Target = T> + 'a {}
impl<'a, T, O: Debug + Deref<Target = T> + 'a> Ref<'a, T> for O {}


type DynUpdate<R> = Update<Box<dyn DynPending<R>>, R>;
type SendFuture<'a, O> = Pin<Box<dyn std::future::Future<Output = O> + Send + 'a>>;
type LocalFuture<'a, O> = Pin<Box<dyn std::future::Future<Output = O> + 'a>>;

#[doc(hidden)]
pub trait DynInstances<C: Contract>: DynClone + Debug + Send + Sync {
    fn create(&self, init: C::Init) -> Box<dyn DynInstance<C>>;
    fn list(&self) -> HashMap<Id, Box<dyn DynInstance<C>>>;
    fn try_next(&mut self) -> Option<Box<dyn DynInstance<C>>>;
    fn next(&mut self) -> SendFuture<'_, Box<dyn DynInstance<C>>>;
}
clone_trait_object!(<C> DynInstances<C> where C: Contract);

impl<C: Contract, I: Instances<C>> DynInstances<C> for I {
    fn create(&self, init: C::Init) -> Box<dyn DynInstance<C>> {Box::new(Instances::create(self, init))}
    fn list(&self) -> HashMap<Id, Box<dyn DynInstance<C>>> {
        Instances::list(self).into_iter().map(|(id, instance)| (id, Box::new(instance) as _)).collect()
    }
    fn try_next(&mut self) -> Option<Box<dyn DynInstance<C>>> {
        Instances::try_next(self).map(|instance| Box::new(instance) as _)
    }
    fn next(&mut self) -> SendFuture<'_, Box<dyn DynInstance<C>>> {
        Box::pin(async move {Box::new(Instances::next(self).await) as _})
    }
}

#[doc(hidden)]
pub trait DynInstance<C: Contract>: DynClone + Debug + Send + Sync {
    fn send(&mut self, msg: C::Message) -> Box<dyn DynPending<C::Result>>;
    fn share(&self, name: Name);
    fn pending(&self) -> Box<dyn Ref<'_, C> + '_>;
    fn confirmed(&self) -> Option<Box<dyn Ref<'_, C> + '_>>;
    fn try_next(&mut self) -> Option<DynUpdate<C::Result>>;
    fn next(&mut self) -> SendFuture<'_, DynUpdate<C::Result>>;
}
clone_trait_object!(<C> DynInstance<C> where C: Contract);

impl<C: Contract, I: Instance<C>> DynInstance<C> for I {
    fn send(&mut self, msg: C::Message) -> Box<dyn DynPending<C::Result>> {
        Box::new(Instance::send(self, msg))
    }
    fn share(&self, name: Name) {Instance::share(self, name)}
    fn pending(&self) -> Box<dyn Ref<'_, C> + '_> {Box::new(Instance::pending(self))}
    fn confirmed(&self) -> Option<Box<dyn Ref<'_, C> + '_>> {
        Instance::confirmed(self).map(|state| Box::new(state) as _)
    }
    fn try_next(&mut self) -> Option<DynUpdate<C::Result>> {
        Instance::try_next(self).map(|update| match update {
            Update::Init => Update::Init,
            Update::Pending(pending) => Update::Pending(Box::new(pending) as _),
            Update::Confirmed(result) => Update::Confirmed(result),
        })
    }
    fn next(&mut self) -> SendFuture<'_, DynUpdate<C::Result>> {
        Box::pin(async move {match Instance::next(self).await {
            Update::Init => Update::Init,
            Update::Pending(pending) => Update::Pending(Box::new(pending) as _),
            Update::Confirmed(result) => Update::Confirmed(result),
        }})
    }
}

#[doc(hidden)]
pub trait DynPending<R: Debug + Clone + Send + Sync + 'static>: DynClone + Debug + Send {
    fn pending(&self) -> Box<dyn Ref<'_, R> + '_>;
    fn try_confirmed(&self) -> Option<R>;
    fn confirmed(self: Box<Self>) -> SendFuture<'static, R>;
    fn try_next(&mut self) -> Option<bool>;
    fn next(&mut self) -> SendFuture<'_, bool>;
}
clone_trait_object!(<R> DynPending<R> where R: Debug + Clone + Send + Sync + 'static);

impl<R: Debug + Clone + Send + Sync + 'static, P: Pending<R>> DynPending<R> for P {
    fn pending(&self) -> Box<dyn Ref<'_, R> + '_> {Box::new(Pending::pending(self))}
    fn try_confirmed(&self) -> Option<R> {Pending::try_confirmed(self)}
    fn confirmed(self: Box<Self>) -> SendFuture<'static, R> {Box::pin(Pending::confirmed(*self))}
    fn try_next(&mut self) -> Option<bool> {Pending::try_next(self)}
    fn next(&mut self) -> SendFuture<'_, bool> {Box::pin(Pending::next(self))}
}

#[doc(hidden)]
pub trait DynPicker<T: Clone + 'static> {
    fn pick(&self) -> Box<dyn DynPicked<T>>;
}
impl<T: Clone + 'static, H: Picker<T>> DynPicker<T> for H {
    fn pick(&self) -> Box<dyn DynPicked<T>> {Box::new(Picker::pick(self))}
}

#[doc(hidden)]
pub trait DynPicked<T: Clone + 'static> {
    fn get(self: Box<Self>) -> LocalFuture<'static, PickResult<T>>;
    fn try_get(&mut self) -> Option<PickResult<T>>;
}

impl<T: Clone + 'static, P: Picked<T>> DynPicked<T> for P {
    fn get(self: Box<Self>) -> LocalFuture<'static, PickResult<T>> {Box::pin(Picked::get(*self))}
    fn try_get(&mut self) -> Option<PickResult<T>> {Picked::try_get(self)}
}

#[doc(hidden)]
pub trait DynHardware: DynClone + DynPicker<Data> + DynPicker<Image> + DynPicker<VideoData>
    + DynPicker<Media> + DynPicker<PathBuf> + DynPicker<Vec<Data>> + DynPicker<Vec<Image>>
    + DynPicker<Vec<VideoData>> + DynPicker<Vec<Media>> + DynPicker<Vec<PathBuf>> {
    fn start_camera(&self) -> Box<dyn Camera>;
    fn video(&self, bytes: Vec<u8>) -> Option<Box<dyn Video>>;
    fn share(&self, data: &Data);
    fn set_clipboard(&self, data: &Data);
    fn get_clipboard(&self) -> Option<Data>;
    fn set_cache(&self, key: &str, data: &Data);
    fn get_cache(&self, key: &str) -> Option<Data>;
    fn trigger_haptic(&self);
}
clone_trait_object!(DynHardware);

impl<H: Hardware> DynHardware for H {
    fn start_camera(&self) -> Box<dyn Camera> {Box::new(Hardware::start_camera(self))}
    fn video(&self, bytes: Vec<u8>) -> Option<Box<dyn Video>> {
        Hardware::video(self, bytes).map(|video| Box::new(video) as _)
    }
    fn share(&self, data: &Data) {Hardware::share(self, data)}
    fn set_clipboard(&self, data: &Data) {Hardware::set_clipboard(self, data)}
    fn get_clipboard(&self) -> Option<Data> {Hardware::get_clipboard(self)}
    fn set_cache(&self, key: &str, data: &Data) {Hardware::set_cache(self, key, data)}
    fn get_cache(&self, key: &str) -> Option<Data> {Hardware::get_cache(self, key)}
    fn trigger_haptic(&self) {Hardware::trigger_haptic(self)}
}
