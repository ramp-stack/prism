use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::traits;

pub type Image = Arc<image::RgbaImage>;
pub type PickResult<T> = Result<T, Cancelled>;

#[derive(Clone, Debug)]
pub enum Data {Text(String), Image(Image), Video(Video), File(PathBuf)}

#[derive(Clone, Debug)]
pub enum Media {Image(Image), Video(Video)}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cancelled;
impl std::fmt::Display for Cancelled {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Selection cancelled")
    }
}
impl std::error::Error for Cancelled {}

pub trait Pickable: Clone + 'static {
    fn pick(hardware: &Hardware) -> Picked<Self>;
}
macro_rules! pickable {
    ($($item:ty),* $(,)?) => {$(
        impl Pickable for $item {
            fn pick(hardware: &Hardware) -> Picked<Self> {
                Picked {picked: traits::DynPicker::<Self>::pick(&*hardware.0), item: None}
            }
        }
    )*};
}
pickable!(
    Data, Image, Video, Media, PathBuf,
    Vec<Data>, Vec<Image>, Vec<Video>, Vec<Media>, Vec<PathBuf>,
);

#[derive(Clone)]
pub struct Hardware(Box<dyn traits::DynHardware>);
impl Hardware {
    pub fn new<H: traits::Hardware>(hardware: H) -> Self {Self(Box::new(hardware))}
    pub fn start_camera(&self) -> Camera {Camera(self.0.start_camera())}
    pub fn pick<T: Pickable>(&self) -> Picked<T> {T::pick(self)}
    pub fn video(&self, bytes: Vec<u8>) -> Option<Video> {self.0.video(bytes).map(Video)}
    pub fn share(&self, data: &Data) {self.0.share(data)}
    pub fn set_clipboard(&self, data: &Data) {self.0.set_clipboard(data)}
    pub fn get_clipboard(&self) -> Option<Data> {self.0.get_clipboard()}
    pub fn set_cache(&self, key: &str, data: &Data) {self.0.set_cache(key, data)}
    pub fn get_cache(&self, key: &str) -> Option<Data> {self.0.get_cache(key)}
    pub fn trigger_haptic(&self) {self.0.trigger_haptic()}
}

#[derive(Clone)]
pub struct Camera(Box<dyn traits::Camera>);
impl Camera {
    pub fn frame(&self) -> Image {self.0.frame()}
}

pub struct Picked<T: Pickable> {
    picked: Box<dyn traits::DynPicked<T>>,
    item: Option<PickResult<T>>,
}
impl<T: Pickable> Picked<T> {
    pub async fn get(self) -> PickResult<T> {
        match self.item {Some(item) => item, None => self.picked.get().await}
    }
    pub fn try_get(&mut self) -> Option<PickResult<T>> {
        if self.item.is_none() {self.item = Some(self.picked.try_get()?);}
        self.item.clone()
    }
}

#[derive(Clone, Debug)]
pub struct Video(Box<dyn traits::Video>);
impl Video {
    pub fn new<V: traits::Video>(video: V) -> Self {Self(Box::new(video))}
    pub fn bytes(&self) -> &Arc<Vec<u8>> {self.0.bytes()}
    pub fn duration(&self) -> Option<Duration> {self.0.duration()}
    pub fn dimensions(&self) -> (u32, u32) {self.0.dimensions()}
    pub fn thumbnail(&self) -> Image {self.0.thumbnail()}
    pub fn format(&self) -> &str {self.0.format()}
    pub fn codec(&self) -> &str {self.0.codec()}
}

#[derive(Clone, Debug)]
pub struct Player(Box<dyn traits::Player>);
impl Player {
    pub fn new(video: &Video) -> Result<Self, String> {video.0.player().map(Self)}
    pub fn start(&self) {self.0.start()}
    pub fn stop(&self) {self.0.stop()}
    pub fn set_speed(&self, speed: f64) {self.0.set_speed(speed)}
    pub fn speed(&self) -> f64 {self.0.speed()}
    pub fn seek(&self, position: Duration) {self.0.seek(position)}
    pub fn position(&self) -> Duration {self.0.position()}
    pub fn duration(&self) -> Option<Duration> {self.0.duration()}
    pub fn is_playing(&self) -> bool {self.0.is_playing()}
    pub fn frame(&self) -> Image {self.0.frame()}
}
