use std::collections::VecDeque;
use std::io::Read;
use std::sync::{Arc, Mutex};

use crate::hardware::Image;
use super::Size;

#[derive(Clone, Debug)]
pub struct Svg(Arc<SvgData>);
impl Svg {
    pub fn read<R: Read>(mut reader: R) -> Result<Self, String> {
        let mut source = String::new();
        reader.read_to_string(&mut source).map_err(|error| error.to_string())?;
        let svg = nsvg::parse_str(&source, nsvg::Units::Pixel, 96.0)
            .map_err(|error| error.to_string())?;
        let dimensions = (svg.width(), svg.height());
        if !dimensions.0.is_finite() || !dimensions.1.is_finite()
            || dimensions.0 <= 0.0 || dimensions.1 <= 0.0 {
            return Err("SVG dimensions must be positive and finite".into())
        }
        Ok(Self(Arc::new(SvgData {source, dimensions, pixels: Mutex::new(VecDeque::new())})))
    }
    pub fn dimensions(&self) -> Size {self.0.dimensions}
    pub fn rasterize(&self, size: (u32, u32)) -> Image {
        assert!(size.0 > 0 && size.1 > 0);
        let mut cache = self.0.pixels.lock().unwrap();
        if let Some(index) = cache.iter().position(|(cached, _)| *cached == size) {
            let entry = cache.remove(index).unwrap();
            let pixels = entry.1.clone();
            cache.push_front(entry);
            return pixels
        }
        let dimensions = self.dimensions();
        let scale = (size.0 as f32 / dimensions.0).min(size.1 as f32 / dimensions.1)
            .max(1.0 / dimensions.0).max(1.0 / dimensions.1);
        let svg = nsvg::parse_str(&self.0.source, nsvg::Units::Pixel, 96.0).unwrap();
        let (width, height, bytes) = svg.rasterize_to_raw_rgba(scale).unwrap();
        let pixels = Image::from_rgba((width, height), bytes);
        let bytes = pixels.pixels().1.len();
        if bytes <= 64 * 1024 * 1024 {
            let mut retained = cache.iter().map(|(_, pixels)| pixels.pixels().1.len()).sum::<usize>();
            while cache.len() >= 4 || retained + bytes > 64 * 1024 * 1024 {
                retained -= cache.pop_back().unwrap().1.pixels().1.len();
            }
            cache.push_front((size, pixels.clone()));
        }
        pixels
    }
}
impl PartialEq for Svg {
    fn eq(&self, other: &Self) -> bool {Arc::ptr_eq(&self.0, &other.0)}
}
impl Eq for Svg {}

#[derive(Debug)]
struct SvgData {
    source: String,
    dimensions: Size,
    pixels: Mutex<VecDeque<((u32, u32), Image)>>,
}
