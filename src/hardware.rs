use futures::FutureExt;
use futures::future::{LocalBoxFuture, Shared};
pub use media::{
    Audio, AudioFrame, MediaFrame, Cursor, Decoder, Encoder, Format, Frame, Image, ImageFormat,
    Media, Pixel, PlaybackMode, Sample, Sound, Tracks, Video, VideoFrame,
};

use std::path::PathBuf;
use std::fmt::{Debug, Formatter};
use std::time::Duration;

use crate::traits;

pub type PickResult<T> = Result<T, Cancelled>;

#[derive(Clone, Debug)]
pub enum Data {Text(String), Image(Image), Media(Media), File(PathBuf)}

/// Video selections retain both tracks; typed `Video` selections contain only images.
#[derive(Clone, Debug)]
pub enum Visual {Image(Image), Video(Media)}

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
    (@item $item:ty) => {
        impl Pickable for $item {
            fn pick(hardware: &Hardware) -> Picked<Self> {
                let mut picked = traits::DynPicker::<Self>::pick(&*hardware.0);
                Picked {
                    result: async move {picked.get().await}.boxed_local().shared(), delivered: false,
                }
            }
        }
    };
    ($($item:ty),* $(,)?) => {$(
        pickable!(@item $item);
        pickable!(@item Vec<$item>);
    )*};
}
pickable!(Data, Image, Media, Visual, Video, Audio, PathBuf);

#[derive(Clone)]
pub struct Hardware(Box<dyn traits::DynHardware>);
impl Hardware {
    pub fn new<H: traits::Hardware>(hardware: H) -> Self {Self(Box::new(hardware))}
    pub fn pick<T: Pickable>(&self) -> Picked<T> {T::pick(self)}
    /// Returns the shared native keyboard capability without showing it.
    pub fn keyboard(&self) -> Keyboard {self.0.keyboard()}
    pub fn player(&self, player: media::Player) -> Result<Player, String> {self.0.player(player)}
    pub fn start_camera(&self) -> Result<Camera, String> {self.0.start_camera()}
    pub fn start_microphone(&self) -> Result<Microphone, String> {self.0.start_microphone()}
    pub fn start_capture(&self) -> Result<Capture, String> {self.0.start_capture()}
    pub fn share(&self, data: &Data) {self.0.share(data)}
    pub fn set_clipboard(&self, data: &Data) {self.0.set_clipboard(data)}
    pub fn get_clipboard(&self) -> Option<Data> {self.0.get_clipboard()}
    pub fn set_cache(&self, key: &str, data: &Data) -> Result<(), String> {self.0.set_cache(key, data)}
    pub fn get_cache(&self, key: &str) -> Option<Data> {self.0.get_cache(key)}
    pub fn trigger_haptic(&self) {self.0.trigger_haptic()}
}

/// Shared native keyboard control. Visibility follows the OS, independently of handle lifetime.
#[derive(Clone)]
pub struct Keyboard {
    keyboard: Box<dyn traits::Keyboard>,
}
impl Keyboard {
    pub fn new<K: traits::Keyboard>(keyboard: K) -> Self {Self {keyboard: Box::new(keyboard)}}
    /// Show the keyboard with a swipe-dismiss region extending this many logical pixels above it.
    pub fn show(&self, dismiss_padding: f32) {self.keyboard.show(dismiss_padding);}
    pub fn hide(&self) {self.keyboard.hide();}
    /// True while any part is visible, including transitions and floating keyboards.
    pub fn visible(&self) -> bool {self.keyboard.visible()}
}
impl Debug for Keyboard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {f.write_str("Keyboard")}
}

#[derive(Clone)]
pub struct Camera {
    capture: Box<dyn traits::Capture<VideoFrame>>,
}
impl Camera {
    pub fn new<C: traits::Capture<VideoFrame>>(capture: C) -> Self {Self {capture: Box::new(capture)}}
    pub async fn next(&mut self) -> Option<VideoFrame> {self.capture.next().await}
    pub fn error(&self) -> Option<String> {self.capture.error()}
}
impl Debug for Camera {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {f.write_str("Camera")}
}

#[derive(Clone)]
pub struct Microphone {
    capture: Box<dyn traits::Capture<AudioFrame>>,
}
impl Microphone {
    pub fn new<C: traits::Capture<AudioFrame>>(capture: C) -> Self {Self {capture: Box::new(capture)}}
    pub async fn next(&mut self) -> Option<AudioFrame> {self.capture.next().await}
    pub fn error(&self) -> Option<String> {self.capture.error()}
}
impl Debug for Microphone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {f.write_str("Microphone")}
}

#[derive(Clone)]
pub struct Capture {
    capture: Box<dyn traits::Capture<MediaFrame>>,
}
impl Capture {
    pub fn new<C: traits::Capture<MediaFrame>>(capture: C) -> Self {Self {capture: Box::new(capture)}}
    pub async fn next(&mut self) -> Option<MediaFrame> {self.capture.next().await}
    pub fn error(&self) -> Option<String> {self.capture.error()}
}
impl Debug for Capture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {f.write_str("Capture")}
}

pub struct Picked<T: Pickable> {
    result: Shared<LocalBoxFuture<'static, PickResult<T>>>,
    delivered: bool,
}
impl<T: Pickable> Picked<T> {
    pub async fn get(&mut self) -> PickResult<T> {self.result.clone().await}
    pub fn try_get(&mut self) -> Option<PickResult<T>> {self.result.clone().now_or_never()}
    /// Deliver this handle's result once; later waits stay pending.
    pub async fn next(&mut self) -> PickResult<T> {
        if self.delivered {return std::future::pending().await}
        let item = self.get().await;
        self.delivered = true;
        item
    }
}
impl<T: Pickable> Clone for Picked<T> {
    fn clone(&self) -> Self {Self {result: self.result.clone(), delivered: false}}
}

#[derive(Clone, Debug)]
pub struct Player {
    playback: Box<dyn traits::Player>,
}
impl Player {
    pub fn new<P: traits::Player>(playback: P) -> Self {Self {playback: Box::new(playback)}}
    pub fn has_video(&self) -> bool {self.playback.has_video()}
    pub fn set_mode(&self, mode: PlaybackMode) {self.playback.set_mode(mode)}
    pub fn mode(&self) -> PlaybackMode {self.playback.mode()}
    pub fn start(&self) {self.playback.start()}
    pub fn stop(&self) {self.playback.stop()}
    pub fn set_speed(&self, speed: f64) {self.playback.set_speed(speed)}
    pub fn speed(&self) -> f64 {self.playback.speed()}
    pub fn set_volume(&self, volume: f64) {self.playback.set_volume(volume)}
    pub fn volume(&self) -> f64 {self.playback.volume()}
    pub fn seek(&self, position: Duration) {self.playback.seek(position)}
    pub fn position(&self) -> Duration {self.playback.position()}
    pub fn duration(&self) -> Option<Duration> {self.playback.duration()}
    pub fn is_playing(&self) -> bool {self.playback.is_playing()}
    pub fn error(&self) -> Option<String> {self.playback.error()}
    /// Load the current snapshot and discard this handle's older unread updates.
    pub fn frame(&self) -> Frame {self.playback.frame()}
    /// Wait for the next queued update without loading or resetting the snapshot baseline.
    pub async fn next(&mut self) -> Frame {self.playback.next().await}
}
