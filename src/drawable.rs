use std::fmt::Debug;
use std::any::Any;

use crate::event::*;
use crate::layout::{SizeRequest, Area, Layout};
use crate::Context;

use wgpu_canvas::{Instruction, Item, Shape, Image, Text};

use downcast_rs::{Downcast, impl_downcast};
pub use dyn_clone::{DynClone, clone_trait_object};

pub use prism_proc::Component;

#[derive(Default, Debug, Clone)]
pub struct RequestTree(pub SizeRequest, pub Vec<RequestTree>);

#[derive(Default, Debug, Clone)]
pub struct SizedTree(pub Size, pub Vec<(Offset, SizedTree)>);

pub type Offset = (f32, f32);
pub type Rect = (f32, f32, f32, f32);
pub type Size = (f32, f32);

/// A renderable element in the UI.
///
/// The `Drawable` trait is implemented by all visual elements
/// such as shapes, text, and images.
#[allow(private_bounds)]
pub trait Drawable: DynClone + Debug + Any + Downcast {
    fn request_size(&self) -> RequestTree;

    fn build(&self, size: Size, request: &RequestTree) -> SizedTree {
        SizedTree(request.0.get(size), vec![])
    }

    fn draw(&self, sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<Instruction>;

    fn name(&self) -> String {std::any::type_name_of_val(self).to_string()}

    fn event(&mut self, _ctx: &mut Context, _sized: &SizedTree, _event: Box<dyn Event>) {}
}

clone_trait_object!(Drawable);
impl_downcast!(Drawable);

impl Drawable for Box<dyn Drawable> {
    fn request_size(&self) -> RequestTree {Drawable::request_size(&**self)}
    fn build(&self, size: Size, request: &RequestTree) -> SizedTree {
        Drawable::build(&**self, size, request)
    }
    fn draw(&self, sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        Drawable::draw(&**self, sized, offset, bound)
    }

    fn name(&self) -> String {Drawable::name(&**self)}

    fn event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) {
        Drawable::event(&mut **self, ctx, sized, event)
    }
}

impl<D: Drawable + Debug + Any + Clone> Drawable for Option<D> {
    fn request_size(&self) -> RequestTree {
        self.as_ref().map(|d| Drawable::request_size(d)).unwrap_or_default()
    }

    fn build(&self, size: Size, request: &RequestTree) -> SizedTree {
        self.as_ref().map(|d| Drawable::build(d, size, request)).unwrap_or_default()
    }

    fn draw(&self, sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
        self.as_ref().map(|d| Drawable::draw(d, sized, offset, bound)).unwrap_or_default()
    }

    fn event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) {
        if let Some(d) = self.as_mut() { Drawable::event(d, ctx, sized, event); }
    }

    fn name(&self) -> String { self.as_ref().map(|d| Drawable::name(d)).unwrap_or("None".to_string()) }
}

/// A composable UI element with children.
///
/// `Component` represents higher-level UI building blocks. 
/// Unlike simple `Drawable`s, components can contain other 
/// drawables and define their own layout, rendering, and event handling.
pub trait Component: Clone + Debug where Self: 'static {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable>;
    fn children(&self) -> Vec<&dyn Drawable>;
    fn layout(&self) -> &dyn Layout;
}

impl<C: Component + Clone + 'static + OnEvent> Drawable for C {
    fn request_size(&self) -> RequestTree {
        let requests = self.children().into_iter().map(Drawable::request_size).collect::<Vec<_>>();
        let info = requests.iter().map(|i| i.0).collect::<Vec<_>>();
        let r = self.layout().request_size(info);
        RequestTree(r, requests)
    }

    fn build(&self, size: Size, request: &RequestTree) -> SizedTree {
        let size = request.0.get(size);
        let children = request.1.iter().map(|b| b.0).collect::<Vec<_>>();
        SizedTree(
            size,
            self.layout().build(size, children).into_iter()
            .zip(self.children()).zip(request.1.iter())
            .map(|((Area{offset, size}, child), branch)| {
                (offset, child.build(size, branch))
            }).collect()
        )
    }

    fn draw(&self, sized: &SizedTree, poffset: Offset, bound: Rect) -> Vec<Instruction> {
        sized.1.iter().zip(self.children()).flat_map(|((offset, branch), child)| {
            let size = branch.0;
            let poffset = (poffset.0+offset.0, poffset.1+offset.1);

            let bound = (
                bound.0.max(poffset.0), bound.1.max(poffset.1),//New bound offset
                bound.2.min((offset.0 + size.0).max(0.0)), bound.3.min((offset.1 + size.1).max(0.0))//New bound size
            );

            if bound.2 != 0.0 && bound.3 != 0.0 {
                child.draw(branch, poffset, bound)
            } else {vec![]}
        }).collect()
    }

    fn event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) {
        let children = sized.1.iter().map(|(o, branch)| Area{offset: *o, size: branch.0}).collect::<Vec<_>>();
        for event in OnEvent::on_event(self, ctx, sized, event) {
            event.pass(ctx, &children).into_iter().zip(self.children_mut()).zip(sized.1.iter()).for_each(
                |((e, child), branch)| if let Some(e) = e {child.event(ctx, &branch.1, e);}
            );
        }
    }
}

#[macro_export]
macro_rules! drawables {
    ( $( $x:expr ),* $(,)? ) => {
        {
            vec![
                $(Box::new($x) as Box<dyn $crate::drawable::Drawable>),*
            ]
        }
    };
}

macro_rules! impl_drawable {
    ( $( $n:ident: $( $x:expr )* ),* ) => {
        $(
            impl Drawable for $n {
                fn request_size(&self) -> RequestTree {
                    RequestTree(SizeRequest::fixed(self.size()), vec![])
                }

                fn draw(&self, _sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<Instruction> {
                    vec![Instruction(wgpu_canvas::Area{offset, bounds: Some(bound)}, ($( $x )*)(self))]
                }
            }
        )*
    };
}
impl_drawable!(
    Item: |s: &Item| s.clone(),
    Shape: |s: &Shape| Item::Shape(*s),
    Text: |s: &Text| Item::Text(s.clone()),
    Image: |s: &Image| Item::Image(s.clone())
);
