use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{DynClone, clone_trait_object};
pub use prism_proc::Component;
pub use wgpu_prism::drawable::{Color, Icon, Instruction, Item, Offset, Rect, Size, Svg, shape, text};
use wgpu_prism::drawable::shape::Variant;

use std::any::Any;
use std::fmt::Debug;
use std::hash::{BuildHasher, Hash};
use std::ops::{Deref, DerefMut};

use crate::Context;
use crate::events::{
    ChangeHandler, ChildEvent, DynEvent, EventHandler, ListenCallback, ListenFuture,
    MouseButton, MouseEvent, MouseState,
};
use crate::hardware::{Image, Player};

#[derive(Default, Debug, Clone)]
pub struct LayoutTree(pub Size, pub Vec<(Offset, LayoutTree)>);

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Area {
    pub offset: Offset,
    pub size: Size,
}

pub trait Layout: std::fmt::Debug + DynClone {
    fn request_width(&mut self, children: Vec<(f32, f32)>) -> (f32, f32);
    fn assign_width(&mut self, width: f32) -> Vec<f32>;
    fn request_height(&mut self, width: f32, children: Vec<(f32, f32)>) -> (f32, f32);
    fn layout(&mut self, size: Size) -> Vec<Area>;
}

impl Layout for Box<dyn Layout> {
    fn request_width(&mut self, children: Vec<(f32, f32)>) -> (f32, f32) {
        (**self).request_width(children)
    }
    fn assign_width(&mut self, width: f32) -> Vec<f32> {(**self).assign_width(width)}
    fn request_height(&mut self, width: f32, children: Vec<(f32, f32)>) -> (f32, f32) {
        (**self).request_height(width, children)
    }
    fn layout(&mut self, size: Size) -> Vec<Area> {(**self).layout(size)}
}
clone_trait_object!(Layout);

pub trait Component: Clone + Debug + 'static {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable>;
    fn children(&self) -> Vec<&dyn Drawable>;
    fn layout(&mut self) -> &mut dyn Layout;
    fn listen(&mut self) -> ListenFuture<'_, Self>;

    fn dispatch_event(&mut self, ctx: &mut Context, event: Box<dyn DynEvent>) -> Vec<Box<dyn DynEvent>> {
        EventHandler::dispatch(self, ctx, event)
    }
}

impl<C: Component> Drawable for C {
    fn request_width(&mut self) -> (f32, f32) {
        let requests = self.children_mut().into_iter().map(Drawable::request_width).collect();
        Component::layout(self).request_width(requests)
    }

    fn request_height(&mut self, width: f32) -> (f32, f32) {
        let widths = Component::layout(self).assign_width(width);
        let requests = self.children_mut().into_iter().zip(widths)
            .map(|(child, width)| child.request_height(width)).collect();
        Component::layout(self).request_height(width, requests)
    }

    fn layout(&mut self, size: Size) -> LayoutTree {
        let areas = Component::layout(self).layout(size);
        LayoutTree(size, areas.into_iter().zip(self.children_mut())
            .map(|(Area {offset, size}, child)| (offset, child.layout(size))).collect())
    }

    fn draw(&self, sized: &LayoutTree, poffset: Offset, bound: Rect) -> Vec<Instruction> {
        sized.1.iter().zip(self.children()).flat_map(|((offset, branch), child)| {
            let size = branch.0;
            let poffset = (poffset.0 + offset.0, poffset.1 + offset.1);

            let left = bound.0.max(poffset.0);
            let top = bound.1.max(poffset.1);
            let bound = (
                left, top, (bound.0 + bound.2).min(poffset.0 + size.0) - left,
                (bound.1 + bound.3).min(poffset.1 + size.1) - top,
            );

            if bound.2 > 0.0 && bound.3 > 0.0 {child.draw(branch, poffset, bound)} else {vec![]}
        }).collect()
    }

    fn event(&mut self, ctx: &mut Context, sized: &LayoutTree, event: Box<dyn DynEvent>) {
        let areas: Vec<_> = sized.1.iter()
            .map(|(offset, branch)| Area {offset: *offset, size: branch.0}).collect();
        for event in Component::dispatch_event(self, ctx, event) {
            let mut events: Vec<Option<Box<dyn DynEvent>>> = areas.iter().map(|_| None).collect();
            let mut slots: Vec<_> = areas.iter().zip(&mut events)
                .map(|(area, event)| ChildEvent {area, event}).collect();
            event.pass_dyn(ctx, &mut slots);
            for ((event, child), (_, branch)) in
                events.into_iter().zip(self.children_mut()).zip(&sized.1) {
                if let Some(event) = event {child.event(ctx, branch, event);}
            }
        }
    }

    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let callback = Component::listen(self).await;
            Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                callback(target.downcast_mut::<Self>().unwrap(), ctx);
            }) as ListenCallback
        })
    }
}

#[allow(private_bounds)]
pub trait Drawable: DynClone + Debug + Any + Downcast {
    fn name(&self) -> String {std::any::type_name_of_val(self).to_string()}
    fn request_width(&mut self) -> (f32, f32);
    fn request_height(&mut self, width: f32) -> (f32, f32);
    fn layout(&mut self, size: Size) -> LayoutTree {LayoutTree(size, vec![])}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction>;
    fn event(&mut self, _ctx: &mut Context, _sized: &LayoutTree, _event: Box<dyn DynEvent>) {}

    fn changed(&mut self) -> ListenFuture<'_> {Box::pin(std::future::pending())}
}
clone_trait_object!(Drawable);
impl_downcast!(Drawable);

impl Drawable for Box<dyn Drawable> {
    fn name(&self) -> String {Drawable::name(&**self)}
    fn request_width(&mut self) -> (f32, f32) {Drawable::request_width(&mut **self)}
    fn request_height(&mut self, width: f32) -> (f32, f32) {
        Drawable::request_height(&mut **self, width)
    }
    fn layout(&mut self, size: Size) -> LayoutTree {Drawable::layout(&mut **self, size)}
    fn draw(&self, sized: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        Drawable::draw(&**self, sized, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, sized: &LayoutTree, event: Box<dyn DynEvent>) {
        Drawable::event(&mut **self, ctx, sized, event)
    }

    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            let callback = (**self).changed().await;
            Box::new(move |target: &mut dyn Any, ctx: &mut Context| {
                callback(target.downcast_mut::<Self>().unwrap().as_mut().as_any_mut(), ctx);
            }) as ListenCallback
        })
    }
}

impl Drawable for Shape {
    fn request_width(&mut self) -> (f32, f32) {(0.0, f32::INFINITY)}
    fn request_height(&mut self, _width: f32) -> (f32, f32) {(0.0, f32::INFINITY)}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = shape::Shape {
            width: layout.0.0, height: layout.0.1, stroke: self.stroke,
            angle: self.angle, color: self.color, variant: self.variant,
        };
        vec![Instruction {offset, bound, item: Item::Shape(shape)}]
    }
}

impl Drawable for Text {
    fn request_width(&mut self) -> (f32, f32) {
        if !self.fit {return (0.0, f32::INFINITY)}
        let width = self.inner.width.take();
        let natural = self.inner.size().0;
        self.inner.width = width;
        (0.0, natural)
    }

    fn request_height(&mut self, width: f32) -> (f32, f32) {
        self.inner.width = Some(width);
        let height = self.inner.size().1;
        (height, height)
    }

    fn draw(&self, _layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        vec![Instruction {offset, bound, item: Item::Text(self.inner.clone())}]
    }

    fn event(&mut self, ctx: &mut Context, _sized: &LayoutTree, event: Box<dyn DynEvent>) {
        EventHandler::dispatch(self, ctx, event);
    }
}

impl Drawable for Image {
    fn request_width(&mut self) -> (f32, f32) {(0.0, f32::INFINITY)}
    fn request_height(&mut self, _width: f32) -> (f32, f32) {(0.0, f32::INFINITY)}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = shape::Shape::new(layout.0.0, layout.0.1, Color::white(), Variant::Rectangle);
        vec![Instruction {offset, bound, item: Item::Image(shape, self.clone(), None)}]
    }
}

impl Drawable for Svg {
    fn request_width(&mut self) -> (f32, f32) {(0.0, f32::INFINITY)}
    fn request_height(&mut self, _width: f32) -> (f32, f32) {(0.0, f32::INFINITY)}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = shape::Shape::new(layout.0.0, layout.0.1, Color::white(), Variant::Rectangle);
        vec![Instruction {offset, bound, item: Item::Svg(shape, self.clone(), None)}]
    }
}

impl Drawable for Player {
    fn request_width(&mut self) -> (f32, f32) {
        if self.has_video() {(0.0, f32::INFINITY)} else {(0.0, 0.0)}
    }
    fn request_height(&mut self, _width: f32) -> (f32, f32) {
        if self.has_video() {(0.0, f32::INFINITY)} else {(0.0, 0.0)}
    }
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        self.frame().video.map_or_else(Vec::new, |frame| frame.image.draw(layout, offset, bound))
    }
    fn event(&mut self, _ctx: &mut Context, _layout: &LayoutTree, event: Box<dyn DynEvent>) {
        if let Some(MouseEvent {position: Some(_), state: MouseState::Pressed(MouseButton::Left)}) =
            event.downcast_ref() {
            if self.is_playing() {self.stop();} else {self.start();}
        }
    }

    fn changed(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            self.next().await;
            Box::new(|_: &mut dyn Any, _: &mut Context| {}) as ListenCallback
        })
    }
}

impl Drawable for Icon {
    fn request_width(&mut self) -> (f32, f32) {self.0.request_width()}
    fn request_height(&mut self, width: f32) -> (f32, f32) {self.0.request_height(width)}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = shape::Shape::new(layout.0.0, layout.0.1, Color::white(), Variant::Rectangle);
        vec![Instruction {offset, bound, item: Item::Svg(shape, self.0.clone(), Some(self.1))}]
    }
}

impl<D: Drawable> Drawables for D {
    fn drawables(&self) -> Vec<&dyn Drawable> {vec![self]}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {vec![self]}
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        Box::pin(async move {
            let callback = self.changed().await;
            Box::new(move |target: &mut Self, ctx: &mut Context| {
                callback(target.as_any_mut(), ctx);
            }) as ListenCallback<Self>
        })
    }
}

pub trait Drawables: 'static {
    fn drawables(&self) -> Vec<&dyn Drawable>;
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable>;
    fn listen_all(&mut self) -> ListenFuture<'_, Self>;
}

impl<D: Drawables> Drawables for Vec<D> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.iter().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.iter_mut().flat_map(Drawables::drawables_mut).collect()
    }
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        Box::pin(async move {
            let callback = self.as_mut_slice().listen_all().await;
            Box::new(move |target: &mut Self, ctx: &mut Context| {
                callback(target.as_mut_slice(), ctx);
            }) as ListenCallback<Self>
        })
    }
}

impl<D: Drawables> Drawables for [D] {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.iter().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.iter_mut().flat_map(Drawables::drawables_mut).collect()
    }
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        let listeners = self.iter_mut().enumerate().map(|(index, child)| {
            let next = child.listen_all();
            Box::pin(async move {
                let callback = next.await;
                Box::new(move |target: &mut Self, ctx: &mut Context| {
                    callback(&mut target[index], ctx);
                }) as ListenCallback<Self>
            }) as ListenFuture<'_, Self>
        }).collect();
        Box::pin(ChangeHandler::select(listeners))
    }
}

impl<D: Drawables, const N: usize> Drawables for [D; N] {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.as_slice().drawables()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {self.as_mut_slice().drawables_mut()}
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        Box::pin(async move {
            let callback = self.as_mut_slice().listen_all().await;
            Box::new(move |target: &mut Self, ctx: &mut Context| {
                callback(target.as_mut_slice(), ctx);
            }) as ListenCallback<Self>
        })
    }
}

impl<D: Drawables> Drawables for Option<D> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.iter().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.iter_mut().flat_map(Drawables::drawables_mut).collect()
    }
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        Box::pin(async move {
            let Some(child) = self.as_mut() else {return std::future::pending().await};
            let callback = child.listen_all().await;
            Box::new(move |target: &mut Self, ctx: &mut Context| {
                callback(target.as_mut().unwrap(), ctx);
            }) as ListenCallback<Self>
        })
    }
}

impl<K: Clone + Eq + Hash + 'static, D: Drawables, S: BuildHasher + 'static>
    Drawables for std::collections::HashMap<K, D, S> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.values().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.values_mut().flat_map(Drawables::drawables_mut).collect()
    }
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        let listeners = self.iter_mut().map(|(key, child)| {
            let key = key.clone();
            let next = child.listen_all();
            Box::pin(async move {
                let callback = next.await;
                Box::new(move |target: &mut Self, ctx: &mut Context| {
                    callback(target.get_mut(&key).unwrap(), ctx);
                }) as ListenCallback<Self>
            }) as ListenFuture<'_, Self>
        }).collect();
        Box::pin(ChangeHandler::select(listeners))
    }
}

impl<K: Clone + Ord + 'static, D: Drawables> Drawables for std::collections::BTreeMap<K, D> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.values().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.values_mut().flat_map(Drawables::drawables_mut).collect()
    }
    fn listen_all(&mut self) -> ListenFuture<'_, Self> {
        let listeners = self.iter_mut().map(|(key, child)| {
            let key = key.clone();
            let next = child.listen_all();
            Box::pin(async move {
                let callback = next.await;
                Box::new(move |target: &mut Self, ctx: &mut Context| {
                    callback(target.get_mut(&key).unwrap(), ctx);
                }) as ListenCallback<Self>
            }) as ListenFuture<'_, Self>
        }).collect();
        Box::pin(ChangeHandler::select(listeners))
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, PartialOrd)]
pub struct SizeRange {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}
impl SizeRange {
    pub fn fixed(size: (f32, f32)) -> Self {Self {
        min_width: size.0, max_width: size.0,
        min_height: size.1, max_height: size.1,
    }}
    pub fn clamp(&self, size: (f32, f32)) -> (f32, f32) {(
        size.0.clamp(self.min_width, self.max_width),
        size.1.clamp(self.min_height, self.max_height),
    )}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shape {
    pub stroke: f32,
    pub angle: f32,
    pub color: Color,
    pub variant: Variant,
}

impl Shape {
    pub fn new(color: Color, variant: Variant) -> Self {Self {stroke: 0.0, angle: 0.0, color, variant}}
}

#[derive(Debug, Clone)]
pub struct Text {
    inner: text::Text,
    fit: bool,
}

impl Text {
    pub fn new(text: &str, font: text::Font, font_size: f32, color: Color, align: text::Align) -> Self {
        Self {inner: text::Text::new(text, font, font_size, color, align), fit: false}
    }

    pub fn fit(mut self) -> Self {self.fit = true; self}
}

impl Deref for Text {
    type Target = text::Text;
    fn deref(&self) -> &Self::Target {&self.inner}
}

impl DerefMut for Text {
    fn deref_mut(&mut self) -> &mut Self::Target {&mut self.inner}
}

macro_rules! tuple_drawables {
    () => {};
    ($type:ident:$field:ident $(, $tail:ident:$rest:ident)*) => {
        tuple_drawables!(@impl ($field, $($rest,)*); $type:$field $(, $tail:$rest)*);
        tuple_drawables!($($tail:$rest),*);
    };
    (@impl $pattern:tt; $($type:ident:$field:ident),+) => {
        impl<$($type: Drawables),+> Drawables for ($($type,)+) {
            fn drawables(&self) -> Vec<&dyn Drawable> {
                let $pattern = self;
                [$($field.drawables()),+].into_iter().flatten().collect()
            }
            fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
                let $pattern = self;
                [$($field.drawables_mut()),+].into_iter().flatten().collect()
            }
            fn listen_all(&mut self) -> ListenFuture<'_, Self> {
                let $pattern = self;
                let listeners: Vec<ListenFuture<'_, Self>> = vec![$({
                    let next = $field.listen_all();
                    Box::pin(async move {
                        let callback = next.await;
                        Box::new(move |target: &mut Self, ctx: &mut Context| {
                            let $pattern = target;
                            callback($field, ctx);
                        }) as ListenCallback<Self>
                    })
                }),+];
                Box::pin(ChangeHandler::select(listeners))
            }
        }
    };
}
tuple_drawables!(A:_a, B:_b, C:_c, D:_d, E:_e, F:_f, G:_g, H:_h, I:_i, J:_j, K:_k, L:_l);
