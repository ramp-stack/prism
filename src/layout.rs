use std::sync::{Mutex, Arc};

#[derive(Clone, Copy, Debug)]
pub struct Area {
    pub offset: (f32, f32),
    pub size: (f32, f32)
}

/// Trait for layouts that determine the offset and allotted sizes of its children
pub trait Layout: std::fmt::Debug {

    /// Given a list of children size requests calculate the size request for the total layout
    fn request_size(&self, children: Vec<SizeRequest>) -> SizeRequest;

    /// Given an allotted size and the list of chlidren size requests (which may respect the size request),
    /// calculate the actual offsets and allotted sizes for its children
    fn build(&self, size: (f32, f32), children: Vec<SizeRequest>) -> Vec<Area>;
}

/// Structure used to designate space to a component or drawable.
///
/// A `SizeRequest` specifies the minimum and maximum dimensions that a
/// component is able to occupy. Layout systems can use this
/// information to determine how to allocate space during rendering.
#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct SizeRequest {
    min_width: f32,
    min_height: f32,
    max_width: f32,
    max_height: f32,
}
impl SizeRequest {
    /// Returns the minimum width.
    pub fn min_width(&self) -> f32 { self.min_width }

    /// Returns the minimum height.
    pub fn min_height(&self) -> f32 { self.min_height }

    /// Returns the maximum width.
    pub fn max_width(&self) -> f32 { self.max_width }

    /// Returns the maximum height.
    pub fn max_height(&self) -> f32 { self.max_height }

    /// Creates a new `SizeRequest`, panicking if min > max for either dimension.
    pub fn new(min_width: f32, min_height: f32, max_width: f32, max_height: f32) -> Self {
        if min_width > max_width { panic!("Min Width was Greater Than Max Width"); }
        if min_height > max_height { panic!("Min Height was Greater Than Max Height"); }
        SizeRequest { min_width, min_height, max_width, max_height }
    }

    /// Creates a fixed-size `SizeRequest` where min and max are equal.
    pub fn fixed(size: (f32, f32)) -> Self {
        SizeRequest { min_width: size.0, min_height: size.1, max_width: size.0, max_height: size.1 }
    }

    /// Creates a `SizeRequest` that can expand to fill all available space.
    pub fn fill() -> Self {
        SizeRequest { min_width: 0.0, min_height: 0.0, max_width: f32::MAX, max_height: f32::MAX }
    }

    /// Clamps a given size into this request's min/max bounds.
    pub fn get(&self, size: (f32, f32)) -> (f32, f32) {
        (
            self.max_width.min(self.min_width.max(size.0)),
            self.max_height.min(self.min_height.max(size.1))
        )
    }

    /// Returns a new request with both width and height increased.
    pub fn add(&self, w: f32, h: f32) -> SizeRequest {
        self.add_width(w).add_height(h)
    }

    /// Returns a new request with width increased.
    pub fn add_width(&self, w: f32) -> SizeRequest {
        SizeRequest::new(self.min_width + w, self.min_height, self.max_width + w, self.max_height)
    }

    /// Returns a new request with height increased.
    pub fn add_height(&self, h: f32) -> SizeRequest {
        SizeRequest::new(self.min_width, self.min_height + h, self.max_width, self.max_height + h)
    }

    /// Returns a new request with height decreased.
    pub fn remove_height(&self, h: f32) -> SizeRequest {
        SizeRequest::new(self.min_width, self.min_height - h, self.max_width, self.max_height - h)
    }

    /// Returns the combined maximum of two requests.
    pub fn max(&self, other: &Self) -> SizeRequest {
        SizeRequest::new(
            self.min_width.max(other.min_width),
            self.min_height.max(other.min_height),
            self.max_width.max(other.max_width),
            self.max_height.max(other.max_height)
        )
    }
}

/// A simple stack layout that overlays children on top of each other.
#[derive(Debug, Clone, Copy)]
pub struct DefaultStack;
impl Layout for DefaultStack {
    fn request_size(&self, children: Vec<SizeRequest>) -> SizeRequest {
        children.into_iter().reduce(|c, o| c.max(&o)).unwrap()
    }

    fn build(&self, size: (f32, f32), children: Vec<SizeRequest>) -> Vec<Area> {
        children.into_iter().map(|c| Area{offset: (0.0, 0.0), size: c.get(size)}).collect()
    }
}

#[derive(Clone, Copy, Default, Debug, PartialEq)]
pub enum Offset {
    #[default]
    Start,
    Center,
    End,
    Static(f32)
}

impl Offset {
    pub fn get(&self, max_size: f32, item_size: f32) -> f32 {
        match self {
            Self::Start => 0.0,
            Self::Center => (max_size - item_size) / 2.0,
            Self::End => max_size - item_size,
            Self::Static(offset) => *offset,
        }
    }

    pub fn size(&self) -> Option<f32> {
        match self {
            Self::Start => Some(0.0),
            Self::Center | Self::End => None,
            Self::Static(offset) => Some(*offset),
        }
    }
}

type CustomFunc = dyn Fn(Vec<(f32, f32)>) -> (f32, f32);
type FitFunc = fn(Vec<(f32, f32)>) -> (f32, f32);

/// Enum specifying how a layout should size and resize its content.
#[derive(Default)]
pub enum Size {
    #[default]
    /// Layout automatically fits the size of its children.
    Fit,
    /// The layout expands to fill the available space but stays within the parent’s maximum size and the minimum size required by its children.    
    Fill,
    /// Layout uses a fixed, static size.
    Static(f32),
    /// Layout size is determined by a custom function.
    Custom(Box<CustomFunc>),
}

impl Size {
    pub fn custom(func: impl Fn(Vec<(f32, f32)>) -> (f32, f32) + 'static) -> Self {
        Size::Custom(Box::new(func))
    }

    pub fn get(&self, items: Vec<(f32, f32)>, fit: FitFunc) -> (f32, f32) {
        match self {
            Size::Fit => fit(items),
            Size::Fill => (items.iter().fold(f32::MIN, |a, b| a.max(b.0)), f32::MAX),
            Size::Static(s) => (*s, *s),
            Size::Custom(f) => f(items)
        }
    }

    pub fn max(items: Vec<(f32, f32)>) -> (f32, f32) {
        items.into_iter().reduce(|s, i| (s.0.max(i.0), s.1.max(i.1))).unwrap_or_default()
    }

    pub fn add(items: Vec<(f32, f32)>) -> (f32, f32) {
        items.into_iter().reduce(|s, i| (s.0+i.0, s.1+i.1)).unwrap_or_default()
    }
}

impl std::fmt::Debug for Size {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Size::Fit => write!(f, "Size::Fit"),
            Size::Fill => write!(f, "Size::Fill"),
            Size::Static(val) => write!(f, "Size::Static({val})"),
            Size::Custom(_) => write!(f, "Size::Custom(<function>)"),
        }
    }
}

/// Structure used to define top, left, bottom, and right padding of an UI element.
///```rust
/// let padding = Padding(24.0, 16.0, 24.0, 16.0);
///```
#[derive(Clone, Debug, Default)]
pub struct Padding(pub f32, pub f32, pub f32, pub f32);

impl Padding {
    pub fn new(p: f32) -> Self {Padding(p, p, p, p)}

    pub fn adjust_size(&self, size: (f32, f32)) -> (f32, f32) {
        let wp = self.0+self.2;
        let hp = self.1+self.3;
        (size.0-wp, size.1-hp)
    }

    pub fn adjust_offset(&self, offset: (f32, f32)) -> (f32, f32) {
        (offset.0+self.0, offset.1+self.1)
    }

    pub fn adjust_request(&self, request: SizeRequest) -> SizeRequest {
        let wp = self.0+self.2;
        let hp = self.1+self.3;
        request.add(wp, hp)
    }
}

pub struct UniformExpand;

impl UniformExpand {
    pub fn get(sizes: Vec<(f32, f32)>, max_size: f32, spacing: f32) -> Vec<f32> {
        // Calculate the total spacing and the minimum size required
        let spacing = (sizes.len() - 1) as f32 * spacing;
        let min_size = sizes.iter().fold(0.0, |s, i| s + i.0) + spacing;

        let mut sizes = sizes.into_iter().map(|s| (s.0, s.1)).collect::<Vec<_>>();

        let mut free_space = (max_size - min_size).max(0.0);
        while free_space > 0.0 {
            let (min_exp, count, next) = sizes.iter().fold((None, 0.0, free_space), |(mut me, mut c, mut ne), size| {
                let min = size.0;
                let max = size.1;
                if min < max { // Item can expand
                    match me {
                        Some(w) if w < min => {
                            ne = ne.min(min - w); // Next size could be the min size of the next expandable block
                        },
                        Some(w) if w == min => {
                            ne = ne.min(max - min); // Next size could be the max size of one of the smallest items
                            c += 1.0;
                        },
                        Some(w) if w > min => {
                            ne = ne.min(max - min).min(w - min); // Next size could be the max size of one of the smallest items
                            me = Some(min);
                            c = 1.0;
                        },
                        _ => {
                            ne = ne.min(max - min); // Next size could be the max size of one of the smallest items
                            me = Some(min);
                            c = 1.0;
                        }
                    }
                }
                (me, c, ne)
            });

            if min_exp.is_none() { break; }
            let min_exp = min_exp.unwrap();

            let expand = (next * count).min(free_space); // Next size could be the rest of the free space
            free_space -= expand;
            let expand = expand / count;

            sizes.iter_mut().for_each(|size| {
                if size.0 < size.1 && size.0 == min_exp {
                    size.0 += expand;
                }
            });
        }

        sizes.into_iter().map(|s| s.0).collect()
    }
}

/// Horizontal layout of items.
///
/// <img src="https://raw.githubusercontent.com/ramp-stack/pelican_ui_std/main/src/examples/row.png"
///      alt="Row Example"
///      width="250">
///
///```rust
/// let layout = Row::new(24.0, Offset::Center, Size::Fit, Padding::new(8.0));
///```
#[derive(Debug, Default)]
pub struct Row(f32, Offset, Size, Padding);

impl Row {
    pub fn new(spacing: f32, offset: Offset, size: Size, padding: Padding) -> Self {
        Row(spacing, offset, size, padding)
    }

    pub fn center(spacing: f32) -> Self {
        Row::new(spacing, Offset::Center, Size::Fit, Padding::default())
    }

    pub fn start(spacing: f32) -> Self {
        Row::new(spacing, Offset::Start, Size::Fit, Padding::default())
    }

    pub fn end(spacing: f32) -> Self {
        Row::new(spacing, Offset::End, Size::Fit, Padding::default())
    }

    pub fn padding(&mut self) -> &mut Padding {&mut self.3}
}

impl Layout for Row {
    fn request_size(&self, children: Vec<SizeRequest>) -> SizeRequest {
        let (widths, heights): (Vec<_>, Vec<_>) = children.into_iter().map(|i|
            ((i.min_width(), i.max_width()), (i.min_height(), i.max_height()))
        ).unzip();
        let spacing = self.0*(widths.len()-1) as f32;
        let width = Size::add(widths);
        let height = self.2.get(heights, Size::max);
        self.3.adjust_request(SizeRequest::new(width.0, height.0, width.1, height.1).add_width(spacing))
    }

    fn build(&self, row_size: (f32, f32), children: Vec<SizeRequest>) -> Vec<Area> {
        let row_size = self.3.adjust_size(row_size);

        let widths = UniformExpand::get(children.iter().map(|i| (i.min_width(), i.max_width())).collect::<Vec<_>>(), row_size.0, self.0);

        let mut offset = 0.0;
        children.into_iter().zip(widths).map(|(i, width)| {
            let size = i.get((width, row_size.1));
            let off = self.3.adjust_offset((offset, self.1.get(row_size.1, size.1)));
            offset += size.0+self.0;
            Area{offset: off, size}
        }).collect()
    }
}

/// Vertical layout of items.
///
/// <img src="https://raw.githubusercontent.com/ramp-stack/pelican_ui_std/main/src/examples/column.png"
///      alt="Column Example"
///      width="250">
///
///```rust
/// let layout = Column::new(24.0, Offset::Center, Size::Fit, Padding::new(8.0));
///```
#[derive(Debug, Default)]
pub struct Column(f32, Offset, Size, Padding, Option<Arc<Mutex<f32>>>);

impl Column {
    pub fn new(spacing: f32, offset: Offset, size: Size, padding: Padding, scrollable: bool) -> Self {
        Column(spacing, offset, size, padding, scrollable.then_some(Arc::new(Mutex::new(0.0))))
    }

    pub fn center(spacing: f32) -> Self {
        Column(spacing, Offset::Center, Size::Fill, Padding::default(), None)
    }

    pub fn start(spacing: f32) -> Self {
        Column(spacing, Offset::Start, Size::Fit, Padding::default(), None)
    }

    pub fn end(spacing: f32) -> Self {
        Column(spacing, Offset::End, Size::Fit, Padding::default(), None)
    }

    pub fn padding(&mut self) -> &mut Padding {&mut self.3}
    pub fn adjust_scroll(&mut self, delta: f32) { if let Some(s) = &mut self.4 { **s.lock().as_mut().unwrap() += delta; } }
    pub fn set_scroll(&mut self, val: f32) { self.4 = Some(Arc::new(Mutex::new(val))); }
}

impl Layout for Column {
    fn request_size(&self, children: Vec<SizeRequest>) -> SizeRequest {
        let (widths, heights): (Vec<_>, Vec<_>) = children.into_iter().map(|i|
            ((i.min_width(), i.max_width()), (i.min_height(), i.max_height()))
        ).unzip();
        let spacing = self.0*(heights.len()-1) as f32;
        let width = self.2.get(widths, Size::max);
        let height = Size::add(heights);
        match self.4.is_some() {
            true => self.3.adjust_request(SizeRequest::new(0.0, 0.0, width.1, height.1).add_height(spacing)),
            false => self.3.adjust_request(SizeRequest::new(width.0, height.0, width.1, height.1).add_height(spacing)),
        }
    }

    fn build(&self, col_size: (f32, f32), children: Vec<SizeRequest>) -> Vec<Area> {
        let col_size = self.3.adjust_size(col_size);
        println!("SIZE {col_size:?} with count {:?}", children.len());
        let heights = UniformExpand::get(children.iter().map(|i| (i.min_height(), i.max_height())).collect::<Vec<_>>(), col_size.1, self.0);
        let mut offset = 0.0;
        children.clone().into_iter().zip(heights).map(|(i, height)| {
            let size = i.get((col_size.0, height));
            let mut off = self.3.adjust_offset((self.1.get(col_size.0, size.0), offset));
            if let Some(sv) = &self.4 {
                let children_height = children.iter().map(|i| i.min_height()).sum::<f32>();
                let content_height = children_height + (self.0 * children.len().saturating_sub(1) as f32);
                let max_scroll = (content_height - col_size.1).max(0.0);

                let mut scroll_val = sv.lock().unwrap();
                *scroll_val = scroll_val.clamp(0.0, max_scroll);
                off.1 -= *scroll_val;
            }
            offset += size.1+self.0;
            Area{offset: off, size}
        }).collect()
    }
}

/// Items stacked on top of each other
///
/// <img src="https://raw.githubusercontent.com/ramp-stack/pelican_ui_std/main/src/examples/stack.png"
///      alt="Stack Example"
///      width="250">
///
///```rust
/// let layout = Stack(Offset::Center, Offset::Center, Size::Fit, Size::Fit, Padding::new(8.0));
///```
#[derive(Debug, Default)]
pub struct Stack(pub Offset, pub Offset, pub Size, pub Size, pub Padding);

impl Stack {
    pub fn new(x_offset: Offset, y_offset: Offset, x_size: Size, y_size: Size, padding: Padding) -> Self {
        Stack(x_offset, y_offset, x_size, y_size, padding)
    }

    pub fn center() -> Self {
        Stack(Offset::Center, Offset::Center, Size::Fit, Size::Fit, Padding::default())
    }

    pub fn start() -> Self {
        Stack(Offset::Start, Offset::Start, Size::Fit, Size::Fit, Padding::default())
    }

    pub fn end() -> Self {
        Stack(Offset::End, Offset::End, Size::Fit, Size::Fit, Padding::default())
    }

    pub fn fill() -> Self {
        Stack(Offset::Center, Offset::Center, Size::Fill, Size::Fill, Padding::default())
    }
}

impl Layout for Stack {
    fn request_size(&self, children: Vec<SizeRequest>) -> SizeRequest {
        let (widths, heights): (Vec<_>, Vec<_>) = children.into_iter().map(|r|
            ((r.min_width(), r.max_width()), (r.min_height(), r.max_height()))
        ).unzip();
        let width = self.2.get(widths, Size::max);
        let height = self.3.get(heights, Size::max);
        self.4.adjust_request(SizeRequest::new(width.0, height.0, width.1, height.1))
    }

    fn build(&self, stack_size: (f32, f32), children: Vec<SizeRequest>) -> Vec<Area> {
        let stack_size = self.4.adjust_size(stack_size);
        children.into_iter().map(|i| {
            let size = i.get(stack_size);
            let offset = (self.0.get(stack_size.0, size.0), self.1.get(stack_size.1, size.1));
            Area{offset: self.4.adjust_offset(offset), size}
        }).collect()
    }
}

/// Horizontal layout that automatically wraps items to the next row when the maximum width is exceeded.
///
/// <img src="https://raw.githubusercontent.com/ramp-stack/pelican_ui_std/main/src/examples/wrap.png"
///      alt="Wrap Example"
///      width="350">
///
///```rust
/// let layout = Wrap::new(8.0, 8.0);
///```
#[derive(Debug)]
pub struct Wrap(pub f32, pub f32, pub Offset, pub Offset, pub Padding, Arc<Mutex<f32>>);

impl Wrap {
    pub fn new(w_spacing: f32, h_spacing: f32) -> Self {
        Wrap(w_spacing, h_spacing, Offset::Center, Offset::Center, Padding::default(), Arc::new(Mutex::new(0.0)))
    }

    pub fn start(w_spacing: f32, h_spacing: f32) -> Self {
        Wrap(w_spacing, h_spacing, Offset::Start, Offset::Center, Padding::default(), Arc::new(Mutex::new(0.0)))
    }

    pub fn end(w_spacing: f32, h_spacing: f32) -> Self {
        Wrap(w_spacing, h_spacing, Offset::End, Offset::Center, Padding::default(), Arc::new(Mutex::new(0.0)))
    }

    pub fn center(w_spacing: f32, h_spacing: f32) -> Self {
        Wrap(w_spacing, h_spacing, Offset::Center, Offset::Center, Padding::default(), Arc::new(Mutex::new(0.0)))
    }
}
impl Layout for Wrap {
    fn request_size(&self, children: Vec<SizeRequest>) -> SizeRequest {
        let mut lw = self.4.1;
        let mut lh = 0.0;
        let mut th = self.4.0;
        let mut max_lw: f32 = 0.0;
        for child in children {
            let (w, h) = (child.min_width(), child.min_height());
            if lw + w > *self.5.lock().unwrap() && lw > self.4.1 {
                th += lh + self.1;
                max_lw = max_lw.max(lw - self.0);
                lw = self.4.1;
                lh = 0.0;
            }
            lw += w + self.0;
            lh = lh.max(h);
        }
        if lw > self.4.1 {
            th += lh;
            max_lw = max_lw.max(lw - self.0);
        }
        SizeRequest::new(max_lw + self.4.2, th + self.4.3, f32::MAX, f32::MAX)
    }

    fn build(&self, maximum_size: (f32, f32), children: Vec<SizeRequest>) -> Vec<Area> {
        *self.5.lock().unwrap() = maximum_size.0;

        let mut areas = Vec::new();
        let mut line = Vec::new();
        let mut tw = self.4.1;
        let mut ho = self.4.0;
        let mut lh = 0.0;

        let flush = |line: &[(f32, f32)], tw: f32, _: f32, ho: f32| {
            if line.is_empty() { return Vec::new(); }
            let line_w = tw - self.0 - self.4.1;
            let extra = (maximum_size.0 - line_w).max(0.0);
            let start_x = match self.2 {
                Offset::Start => self.4.1,
                Offset::End => self.4.1 + extra,
                Offset::Center => self.4.1 + extra / 2.0,
                Offset::Static(_) => 0.0,
            };
            let mut x = start_x;
            line.iter().map(|&(w, h)| {
                let a = Area { offset: (x, ho), size: (w, h) };
                x += w + self.0;
                a
            }).collect()
        };

        for child in children {
            let (w, h) = (child.min_width(), child.min_height());
            if tw + w > maximum_size.0 && tw > self.4.1 {
                areas.extend(flush(&line, tw, lh, ho));
                ho += lh + self.1;
                tw = self.4.1;
                lh = 0.0;
                line.clear();
            }
            line.push((w, h));
            tw += w + self.0;
            lh = lh.max(h);
        }
        areas.extend(flush(&line, tw, lh, ho));
        areas
    }
}

/// Defines the reference point for scrolling content.
#[derive(Debug, Clone, Copy)]
pub enum ScrollAnchor {
    Start,
    End,
}