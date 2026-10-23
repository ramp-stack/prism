use futures::FutureExt;
use futures::future::{LocalBoxFuture, Shared};

use std::borrow::Borrow;
use std::path::PathBuf;
use std::io::{Read, Seek, Write};
use std::iter::FusedIterator;
use std::ops::RangeBounds;
use std::fmt::{Debug, Formatter};
use std::sync::Arc;
use std::time::Duration;

use crate::{ContentBackend, traits};

pub type PickResult<T> = Result<T, Cancelled>;

#[derive(Clone, Debug)]
pub struct Sound {
    pub sample_rate: u32,
    pub channels: u16,
    pub samples: Arc<Vec<f32>>,
}
impl Sound {
    pub fn duration(&self) -> Duration {
        let frames = self.samples.len() / self.channels as usize;
        Duration::from_secs_f64(frames as f64 / self.sample_rate as f64)
    }
}

#[derive(Clone, Debug)]
pub enum Data {Text(String), Image(Image), Media(Media), File(PathBuf)}

#[derive(Clone, Debug)]
pub enum Visual {Image(Image), Video(Video)}

/// Picker result for a video asset, retaining its audio track when present.
#[derive(Clone, Debug)]
pub struct Video(pub Media);
impl AsRef<Media> for Video {
    fn as_ref(&self) -> &Media {&self.0}
}

/// Picker result for an asset with audio and no video track.
#[derive(Clone, Debug)]
pub struct Audio(pub Media);
impl AsRef<Media> for Audio {
    fn as_ref(&self) -> &Media {&self.0}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Tracks {Audio, Video, Both}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ImageFormat {Png, Jpeg, Webp, Gif, Bmp, Ico, Tiff, Qoi}
impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Png => "png", Self::Jpeg => "jpg", Self::Webp => "webp", Self::Gif => "gif",
            Self::Bmp => "bmp", Self::Ico => "ico", Self::Tiff => "tiff", Self::Qoi => "qoi",
        }
    }
}

#[allow(non_camel_case_types)]
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum Format {
    Mp4_Mpeg4p2_Aac, Mp4_Mpeg4p2,
    Mkv_Ffv1_Flac, Mkv_Ffv1_PcmF32le, Mkv_Ffv1,
    Mov_ProRes422_PcmS16le, Mov_ProRes422,
    M4a_Aac, M4a_Alac, Wav_PcmS16le, Wav_PcmF32le, Flac_Flac,
}
impl Format {
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Mp4_Mpeg4p2_Aac | Self::Mp4_Mpeg4p2 => "mp4",
            Self::Mkv_Ffv1_Flac | Self::Mkv_Ffv1_PcmF32le | Self::Mkv_Ffv1 => "mkv",
            Self::Mov_ProRes422_PcmS16le | Self::Mov_ProRes422 => "mov",
            Self::M4a_Aac | Self::M4a_Alac => "m4a",
            Self::Wav_PcmS16le | Self::Wav_PcmF32le => "wav",
            Self::Flac_Flac => "flac",
        }
    }
    pub fn audio(&self) -> bool {
        !matches!(self, Self::Mp4_Mpeg4p2 | Self::Mkv_Ffv1 | Self::Mov_ProRes422)
    }
    pub fn video(&self) -> bool {
        match self {
            Self::Mp4_Mpeg4p2_Aac | Self::Mp4_Mpeg4p2 | Self::Mkv_Ffv1_Flac
                | Self::Mkv_Ffv1_PcmF32le | Self::Mkv_Ffv1
                | Self::Mov_ProRes422_PcmS16le | Self::Mov_ProRes422 => true,
            _ => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PlaybackMode {#[default] Default, Live(Duration), Loop}

#[derive(Clone, Debug)]
pub struct Frame {
    pub image: Option<Image>,
    pub position: Duration,
    pub duration: Duration,
    pub mode: PlaybackMode,
    pub speed: f64,
    pub volume: f64,
    pub playing: bool,
    pub error: Option<String>,
}

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
    pub fn open_keyboard(&self) -> Keyboard {self.0.open_keyboard()}
    pub fn start_camera(&self) -> Result<Camera, String> {self.0.start_camera()}
    pub fn start_microphone(&self) -> Result<Microphone, String> {self.0.start_microphone()}
    pub fn start_capture(&self) -> Result<Capture, String> {self.0.start_capture()}
    pub fn share(&self, data: &Data) {self.0.share(data)}
    pub fn set_clipboard(&self, data: &Data) {self.0.set_clipboard(data)}
    pub fn get_clipboard(&self) -> Option<Data> {self.0.get_clipboard()}
    pub fn set_cache(&self, key: &str, data: &Data) -> Result<(), String> {self.0.set_cache(key, data)}
    pub fn get_cache(&self, key: &str) -> Result<Option<Data>, String> {self.0.get_cache(key)}
    pub fn trigger_haptic(&self) {self.0.trigger_haptic()}
}

/// Retains a native keyboard session. Dropping the last clone requests dismissal.
#[derive(Clone)]
pub struct Keyboard {
    _session: Box<dyn traits::Keyboard>,
}
impl Keyboard {
    pub fn new<K: traits::Keyboard>(session: K) -> Self {Self {_session: Box::new(session)}}
}
impl Debug for Keyboard {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {f.write_str("Keyboard")}
}

#[derive(Clone)]
pub struct Camera {
    media: Media,
    capture: Box<dyn traits::Capture>,
}
impl Camera {
    pub fn new<C: traits::Capture>(media: Media, capture: C) -> Self {
        Self {media, capture: Box::new(capture)}
    }
    pub fn error(&self) -> Option<String> {self.capture.error()}
}
impl AsRef<Media> for Camera {
    fn as_ref(&self) -> &Media {&self.media}
}
impl Debug for Camera {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Camera").field(&self.media).finish()
    }
}

#[derive(Clone)]
pub struct Microphone {
    media: Media,
    capture: Box<dyn traits::Capture>,
}
impl Microphone {
    pub fn new<C: traits::Capture>(media: Media, capture: C) -> Self {
        Self {media, capture: Box::new(capture)}
    }
    pub fn error(&self) -> Option<String> {self.capture.error()}
}
impl AsRef<Media> for Microphone {
    fn as_ref(&self) -> &Media {&self.media}
}
impl Debug for Microphone {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Microphone").field(&self.media).finish()
    }
}

#[derive(Clone)]
pub struct Capture {
    media: Media,
    capture: Box<dyn traits::Capture>,
}
impl Capture {
    pub fn new<C: traits::Capture>(media: Media, capture: C) -> Self {
        Self {media, capture: Box::new(capture)}
    }
    pub fn error(&self) -> Option<String> {self.capture.error()}
}
impl AsRef<Media> for Capture {
    fn as_ref(&self) -> &Media {&self.media}
}
impl Debug for Capture {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Capture").field(&self.media).finish()
    }
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
pub struct Image(Box<dyn traits::DynImage>);
impl Image {
    pub fn read<R: Read>(mut reader: R) -> Result<Self, String> {
        (ContentBackend::get().read_image)(&mut reader)
    }
    pub fn from_rgba(dimensions: (u32, u32), pixels: Vec<u8>) -> Self {
        (ContentBackend::get().image_from_rgba)(dimensions, pixels)
    }
    pub fn downcast_ref<I: traits::Image>(&self) -> Option<&I> {self.0.downcast_ref()}
    pub fn dimensions(&self) -> (u32, u32) {self.0.dimensions()}
    /// Borrow decoded RGBA8 bytes with their actual dimensions, including corruption placeholders.
    pub fn pixels(&self) -> ((u32, u32), &[u8]) {self.0.pixels()}
    pub fn write<W: Write>(&self, mut writer: W, format: ImageFormat) -> Result<(), String> {
        self.0.write(&mut writer, format)
    }
}
impl<I: traits::Image> From<I> for Image {
    fn from(image: I) -> Self {Self(Box::new(image))}
}
impl PartialEq for Image {
    fn eq(&self, other: &Self) -> bool {self.0.as_ref().eq(other)}
}
impl Eq for Image {}

#[derive(Clone, Debug)]
pub struct Media(Box<dyn traits::DynMedia>);
impl Media {
    pub fn new() -> Self {(ContentBackend::get().media)()}
    pub fn read<R: Read + Seek>(mut reader: R) -> Result<Self, String> {
        (ContentBackend::get().read_media)(&mut reader)
    }
    pub fn downcast_ref<T: traits::Media>(&self) -> Option<&T> {self.0.downcast_ref()}
    pub fn copy<R: RangeBounds<Duration>>(&self, range: R, tracks: Tracks) -> Result<Self, String> {
        let range = (range.start_bound().cloned(), range.end_bound().cloned());
        self.0.copy(range, tracks).map(Self)
    }
    pub fn replace<R: RangeBounds<Duration>>(&self, range: R, replacement: &Self, tracks: Tracks) -> Result<(), String> {
        let range = (range.start_bound().cloned(), range.end_bound().cloned());
        self.0.replace(range, replacement, tracks)
    }
    pub fn write<W: Write + Seek>(&self, mut writer: W, format: Format) -> Result<(), String> {
        self.0.write(&mut writer, format)
    }
    pub fn duration(&self) -> Duration {self.0.duration()}
    pub fn append(&self, other: &Self) {self.0.append(other)}
    pub fn chunks(&self, duration: Duration) -> Result<Chunks, String> {self.0.chunks(duration)}
    pub fn dimensions(&self) -> Option<(u32, u32)> {self.0.dimensions()}
    pub fn thumbnail(&self) -> Option<Image> {self.0.thumbnail()}
}
impl<M: traits::Media> From<M> for Media {
    fn from(media: M) -> Self {Self(Box::new(media))}
}
impl Default for Media {
    fn default() -> Self {Self::new()}
}
impl TryFrom<Sound> for Media {
    type Error = String;
    fn try_from(sound: Sound) -> Result<Self, String> {(ContentBackend::get().media_from_sound)(sound)}
}
impl TryFrom<(Image, Duration)> for Media {
    type Error = String;
    fn try_from(image: (Image, Duration)) -> Result<Self, String> {
        (ContentBackend::get().media_from_image)(image)
    }
}
impl<T: Borrow<Media>> Extend<T> for Media {
    fn extend<I: IntoIterator<Item = T>>(&mut self, items: I) {
        for item in items {self.append(item.borrow());}
    }
}
impl<T: Borrow<Media>> FromIterator<T> for Media {
    fn from_iter<I: IntoIterator<Item = T>>(items: I) -> Self {
        let mut media = Self::new();
        media.extend(items);
        media
    }
}
impl AsRef<Media> for Media {
    fn as_ref(&self) -> &Media {self}
}

#[derive(Clone, Debug)]
pub struct Chunks(Box<dyn traits::Chunks>);
impl Chunks {
    pub fn new<I: traits::Chunks>(chunks: I) -> Self {Self(Box::new(chunks))}
}
impl Iterator for Chunks {
    type Item = Media;
    fn next(&mut self) -> Option<Media> {self.0.next()}
    fn nth(&mut self, n: usize) -> Option<Media> {self.0.nth(n)}
    fn size_hint(&self) -> (usize, Option<usize>) {self.0.size_hint()}
    fn count(self) -> usize {self.0.len()}
    fn last(mut self) -> Option<Media> {self.0.next_back()}
}
impl DoubleEndedIterator for Chunks {
    fn next_back(&mut self) -> Option<Media> {self.0.next_back()}
    fn nth_back(&mut self, n: usize) -> Option<Media> {self.0.nth_back(n)}
}
impl ExactSizeIterator for Chunks {
    fn len(&self) -> usize {self.0.len()}
}
impl FusedIterator for Chunks {}

#[derive(Clone, Debug)]
pub struct Player {
    playback: Box<dyn traits::Player>,
    source: Media,
}
impl Player {
    pub fn new(source: impl AsRef<Media>) -> Result<Self, String> {
        let source = source.as_ref().clone();
        let playback = source.0.player()?;
        Ok(Self {playback, source})
    }
    pub fn source(&self) -> &Media {&self.source}
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
    pub fn duration(&self) -> Duration {self.playback.duration()}
    pub fn is_playing(&self) -> bool {self.playback.is_playing()}
    pub fn error(&self) -> Option<String> {self.playback.error()}
    /// Load the current snapshot and discard this handle's older unread updates.
    pub fn frame(&self) -> Frame {self.playback.frame()}
    /// Wait for the next queued update without loading or resetting the snapshot baseline.
    pub async fn next(&mut self) -> Frame {self.playback.next().await}
}
