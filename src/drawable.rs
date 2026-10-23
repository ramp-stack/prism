use downcast_rs::{Downcast, impl_downcast};
use dyn_clone::{DynClone, clone_trait_object};
use futures::future::LocalBoxFuture;
pub use prism_proc::Component;

use std::any::Any;
use std::fmt::Debug;

use crate::Context;
use crate::events::{
    ChildEvent, DynEvent, EventHandler, ListenFuture, MouseButton, MouseEvent, MouseState,
};
use crate::hardware::{Image, Player};

pub mod shape;
use shape::{Shape, Variant};
pub mod text;
mod image;
pub use image::Svg;

pub type Offset = (f32, f32);
pub type Rect = (f32, f32, f32, f32);
pub type Size = (f32, f32);

pub enum Item {
    Shape(Shape),
    Text(text::Text),
    Image(Shape, Image, Option<Color>),
    Svg(Shape, Svg, Option<Color>),
}

pub struct Instruction {
    pub bound: Rect,
    pub offset: Offset,
    pub item: Item,
}

/// Unpremultiplied sRGB bytes with a linear alpha byte (0 transparent, 255 opaque).
#[derive(Default, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Color(pub u8, pub u8, pub u8, pub u8);
impl Color {
    pub fn white() -> Self {Color(255, 255, 255, 255)}
    pub fn black() -> Self {Color(0, 0, 0, 255)}
    pub fn transparent() -> Self {Color(0, 0, 0, 0)}
    pub fn red() -> Self {Color(255, 0, 0, 255)}
    pub fn green() -> Self {Color(0, 255, 0, 255)}
    pub fn blue() -> Self {Color(0, 0, 255, 255)}
    pub fn yellow() -> Self {Color(255, 255, 0, 255)}
    pub fn magenta() -> Self {Color(255, 0, 255, 255)}
    pub fn cyan() -> Self {Color(0, 255, 255, 255)}
}

#[derive(Debug, Clone)]
pub struct Icon(pub Svg, pub Color);

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

#[derive(Default, Debug, Clone, PartialEq)]
pub struct RangeTree(pub SizeRange, pub Vec<RangeTree>);

#[derive(Default, Debug, Clone)]
pub struct LayoutTree(pub Size, pub Vec<(Offset, LayoutTree)>);

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Area {
    pub offset: Offset,
    pub size: Size,
}

pub trait Component: Clone + Debug + 'static {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable>;
    fn children(&self) -> Vec<&dyn Drawable>;
    fn layout(&self) -> &dyn Layout;
    fn listen(&mut self) -> ListenFuture<'_>;
    fn update(&mut self, ctx: &mut Context, path: Vec<usize>, update: Box<dyn Any>);

    fn dispatch_event(&mut self, ctx: &mut Context, event: Box<dyn DynEvent>) -> Vec<Box<dyn DynEvent>> {
        EventHandler::dispatch(self, ctx, event)
    }
}

pub trait Layout: std::fmt::Debug + DynClone {
    fn size_range(&self, children: Vec<SizeRange>) -> SizeRange;
    fn layout(&self, size: Size, children: Vec<SizeRange>) -> Vec<Area>;
}

impl Layout for Box<dyn Layout> {
    fn size_range(&self, children: Vec<SizeRange>) -> SizeRange {(**self).size_range(children)}
    fn layout(&self, size: Size, children: Vec<SizeRange>) -> Vec<Area> {(**self).layout(size, children)}
}
clone_trait_object!(Layout);

#[allow(private_bounds)]
pub trait Drawable: DynClone + Debug + Any + Downcast {
    fn name(&self) -> String {std::any::type_name_of_val(self).to_string()}
    fn size_range(&self) -> RangeTree;
    fn layout(&mut self, size: Size, range_tree: &RangeTree) -> LayoutTree {
        LayoutTree(range_tree.0.clamp(size), vec![])
    }
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction>;
    fn event(&mut self, _ctx: &mut Context, _sized: &LayoutTree, _event: Box<dyn DynEvent>) {}

    fn listen(&mut self) -> ListenFuture<'_> {Box::pin(std::future::pending())}
    fn update(&mut self, _ctx: &mut Context, _path: Vec<usize>, _update: Box<dyn Any>) {
        unreachable!("drawable has no change listener")
    }
    fn changed<'a>(&'a mut self, ctx: &'a mut Context) -> LocalBoxFuture<'a, ()> {
        Box::pin(async move {
            let (path, update) = self.listen().await;
            self.update(ctx, path, update);
        })
    }
}
clone_trait_object!(Drawable);
impl_downcast!(Drawable);

impl<C: Component> Drawable for C {
    fn listen(&mut self) -> ListenFuture<'_> {Component::listen(self)}
    fn update(&mut self, ctx: &mut Context, path: Vec<usize>, update: Box<dyn Any>) {
        Component::update(self, ctx, path, update);
    }

    fn size_range(&self) -> RangeTree {
        let requests = self.children().into_iter().map(Drawable::size_range).collect::<Vec<_>>();
        let info = requests.iter().map(|i| i.0).collect::<Vec<_>>();
        let r = Component::layout(self).size_range(info);
        RangeTree(r, requests)
    }

    fn layout(&mut self, size: Size, request: &RangeTree) -> LayoutTree {
        let size = request.0.clamp(size);
        let children = request.1.iter().map(|b| b.0).collect::<Vec<_>>();
        LayoutTree(size, Component::layout(self).layout(size, children).into_iter()
            .zip(self.children_mut()).zip(request.1.iter())
            .map(|((Area {offset, size}, child), branch)| (offset, child.layout(size, branch)))
            .collect())
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
}

impl Drawable for Box<dyn Drawable> {
    fn listen(&mut self) -> ListenFuture<'_> {(**self).listen()}
    fn update(&mut self, ctx: &mut Context, path: Vec<usize>, update: Box<dyn Any>) {
        (**self).update(ctx, path, update);
    }

    fn name(&self) -> String {Drawable::name(&**self)}
    fn size_range(&self) -> RangeTree {Drawable::size_range(&**self)}
    fn layout(&mut self, size: Size, request: &RangeTree) -> LayoutTree {
        Drawable::layout(&mut **self, size, request)
    }
    fn draw(&self, sized: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        Drawable::draw(&**self, sized, offset, bound)
    }
    fn event(&mut self, ctx: &mut Context, sized: &LayoutTree, event: Box<dyn DynEvent>) {
        Drawable::event(&mut **self, ctx, sized, event)
    }
}

impl Drawable for Shape {
    fn size_range(&self) -> RangeTree {RangeTree(SizeRange::fixed(self.size()), vec![])}
    fn draw(&self, _layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        vec![Instruction {offset, bound, item: Item::Shape(*self)}]
    }
}

impl Drawable for text::Text {
    fn size_range(&self) -> RangeTree {RangeTree(SizeRange::fixed(self.size()), vec![])}
    fn draw(&self, _layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        vec![Instruction {offset, bound, item: Item::Text(self.clone())}]
    }
}

impl Drawable for Image {
    fn size_range(&self) -> RangeTree {
        RangeTree(SizeRange {
            min_width: 0.0, max_width: f32::INFINITY,
            min_height: 0.0, max_height: f32::INFINITY,
        }, vec![])
    }
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = Shape::new(layout.0.0, layout.0.1, Color::white(), Variant::Rectangle);
        vec![Instruction {offset, bound, item: Item::Image(shape, self.clone(), None)}]
    }
}

impl Drawable for Svg {
    fn size_range(&self) -> RangeTree {
        RangeTree(SizeRange {
            min_width: 0.0, max_width: f32::INFINITY,
            min_height: 0.0, max_height: f32::INFINITY,
        }, vec![])
    }
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = Shape::new(layout.0.0, layout.0.1, Color::white(), Variant::Rectangle);
        vec![Instruction {offset, bound, item: Item::Svg(shape, self.clone(), None)}]
    }
}

impl Drawable for Player {
    fn listen(&mut self) -> ListenFuture<'_> {
        Box::pin(async move {
            self.next().await;
            (vec![], Box::new(()) as Box<dyn Any>)
        })
    }

    fn update(&mut self, _ctx: &mut Context, path: Vec<usize>, update: Box<dyn Any>) {
        assert!(path.is_empty() && update.is::<()>());
    }

    fn size_range(&self) -> RangeTree {
        if self.source().dimensions().is_some() {
            RangeTree(SizeRange {
                min_width: 0.0, max_width: f32::INFINITY,
                min_height: 0.0, max_height: f32::INFINITY,
            }, vec![])
        } else {RangeTree(SizeRange::fixed((0.0, 0.0)), vec![])}
    }
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        self.frame().image.map_or_else(Vec::new, |image| image.draw(layout, offset, bound))
    }
    fn event(&mut self, _ctx: &mut Context, _layout: &LayoutTree, event: Box<dyn DynEvent>) {
        if let Some(MouseEvent {position: Some(_), state: MouseState::Pressed(MouseButton::Left)}) =
            event.downcast_ref() {
            if self.is_playing() {self.stop();} else {self.start();}
        }
    }
}

impl Drawable for Icon {
    fn size_range(&self) -> RangeTree {self.0.size_range()}
    fn draw(&self, layout: &LayoutTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        let shape = Shape::new(layout.0.0, layout.0.1, Color::white(), Variant::Rectangle);
        vec![Instruction {offset, bound, item: Item::Svg(shape, self.0.clone(), Some(self.1))}]
    }
}

pub trait Drawables {
    fn drawables(&self) -> Vec<&dyn Drawable>;
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable>;
}

impl<D: Drawable> Drawables for D {
    fn drawables(&self) -> Vec<&dyn Drawable> {vec![self]}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {vec![self]}
}

impl<D: Drawables> Drawables for Vec<D> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.iter().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.iter_mut().flat_map(Drawables::drawables_mut).collect()
    }
}

impl<D: Drawables> Drawables for [D] {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.iter().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.iter_mut().flat_map(Drawables::drawables_mut).collect()
    }
}

impl<D: Drawables, const N: usize> Drawables for [D; N] {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.as_slice().drawables()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {self.as_mut_slice().drawables_mut()}
}

impl<D: Drawables> Drawables for Option<D> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.iter().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.iter_mut().flat_map(Drawables::drawables_mut).collect()
    }
}

impl<K, D: Drawables, S> Drawables for std::collections::HashMap<K, D, S> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.values().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.values_mut().flat_map(Drawables::drawables_mut).collect()
    }
}

impl<K, D: Drawables> Drawables for std::collections::BTreeMap<K, D> {
    fn drawables(&self) -> Vec<&dyn Drawable> {self.values().flat_map(Drawables::drawables).collect()}
    fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
        self.values_mut().flat_map(Drawables::drawables_mut).collect()
    }
}

macro_rules! tuple_drawables {
    () => {};
    ($type:ident:$field:ident $(, $tail:ident:$rest:ident)*) => {
        impl<$type: Drawables, $($tail: Drawables),*> Drawables for ($type, $($tail,)*) {
            fn drawables(&self) -> Vec<&dyn Drawable> {
                let ($field, $($rest,)*) = self;
                [$field.drawables(), $($rest.drawables()),*].into_iter().flatten().collect()
            }
            fn drawables_mut(&mut self) -> Vec<&mut dyn Drawable> {
                let ($field, $($rest,)*) = self;
                [$field.drawables_mut(), $($rest.drawables_mut()),*].into_iter().flatten().collect()
            }
        }
        tuple_drawables!($($tail:$rest),*);
    };
}
tuple_drawables!(A:a, B:b, C:c, D:d, E:e, F:f, G:g, H:h, I:i, J:j, K:k, L:l);
