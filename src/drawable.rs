use std::fmt::Debug;
use std::any::Any;

use crate::event::*;
use crate::layout::{SizeRequest, Area, Layout};
use crate::Context;

use wgpu_canvas::{Image, Shape, Text, Area as CanvasArea, Item as CanvasItem};

use downcast_rs::{Downcast, impl_downcast};

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
pub trait Drawable: Debug + Any + Downcast {
    fn request_size(&self) -> RequestTree;

    fn build(&self, size: Size, request: RequestTree) -> SizedTree {
        SizedTree(request.0.get(size), vec![])
    }
    fn draw(&self, sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)>;

    fn name(&self) -> String {std::any::type_name_of_val(self).to_string()}

    fn event(&mut self, _ctx: &mut Context, _sized: &SizedTree, _event: Box<dyn Event>) {}
}


// 1. drawables should have requist size and build/draw fn
// 2. event needs to accept mut root (define what a root is) instead of self or context
// 3. context should be root (root has inner drawable)
// 4. components should return a layout and the implementation of drawable should use the layout directly
//     change the component macro to return the layout;
    
// 6. creat root object that cand do everything that we want it to do then give it cookies


impl_downcast!(Drawable);

impl Drawable for Box<dyn Drawable> {
    fn request_size(&self) -> RequestTree {Drawable::request_size(&**self)}
    fn build(&self, size: Size, request: RequestTree) -> SizedTree {
        Drawable::build(&**self, size, request)
    }
    fn draw(&self, sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)> {
        Drawable::draw(&**self, sized, offset, bound)
    }

    fn name(&self) -> String {Drawable::name(&**self)}

    fn event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) {
        Drawable::event(&mut **self, ctx, sized, event)
    }
}

impl<D: Drawable + Debug + Any> Drawable for Option<D> {
    fn request_size(&self) -> RequestTree {
        self.as_ref().map(|d| d.request_size()).unwrap_or_default()
    }

    fn build(&self, size: Size, request: RequestTree) -> SizedTree {
        self.as_ref().map(|d| d.build(size, request)).unwrap_or_default()
    }

    fn draw(&self, sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)> {
        self.as_ref().map(|d| d.draw(sized, offset, bound)).unwrap_or_default()
    }

    fn name(&self) -> String { self.as_ref().map(|d| d.name()).unwrap_or("None".to_string()) }

    fn event(&mut self, ctx: &mut Context, sized: &SizedTree, event: Box<dyn Event>) {
        if let Some(d) = self { d.event(ctx, sized, event); }
    }
}

impl Drawable for Text {
    fn request_size(&self) -> RequestTree {
        RequestTree(SizeRequest::fixed(self.size()), vec![])
    }

    fn draw(&self, _sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)> {
        vec![(CanvasArea{offset, bounds: Some(bound)}, CanvasItem::Text(self.clone()))]
    }
}

impl Drawable for Shape {
    fn request_size(&self) -> RequestTree {
        RequestTree(SizeRequest::fixed(self.shape.size()), vec![])
    }

    fn draw(&self, _sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)> {
        vec![(CanvasArea{offset, bounds: Some(bound)}, CanvasItem::Shape(*self))]
    }
}

impl Drawable for Image {
    fn request_size(&self) -> RequestTree {
        RequestTree(SizeRequest::fixed(self.shape.size()), vec![])
    }

    fn draw(&self, _sized: &SizedTree, offset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)> {
        vec![(CanvasArea{offset, bounds: Some(bound)}, CanvasItem::Image(self.clone()))]
    }
}

/// A composable UI element with children.
///
/// `Component` represents higher-level UI building blocks. 
/// Unlike simple `Drawable`s, components can contain other 
/// drawables and define their own layout, rendering, and event handling.
pub trait Component: Debug where Self: 'static {
    fn children_mut(&mut self) -> Vec<&mut dyn Drawable>;
    fn children(&self) -> Vec<&dyn Drawable>;
    fn layout(&self) -> &dyn Layout;
}
//TODO: could relpaces request_size and build with a layout getter and run Layout methods directly

impl<C: Component + 'static + OnEvent> Drawable for C {
    fn request_size(&self) -> RequestTree {
        let timer = std::time::Instant::now();
        let requests = self.children().into_iter().map(Drawable::request_size).collect::<Vec<_>>();
        println!("Drawable::request_size took: {:?}", timer.elapsed().as_nanos());
        let timer = std::time::Instant::now();
        let info = requests.iter().map(|i| i.0).collect::<Vec<_>>();
        println!("Collecting requests took: {:?}", timer.elapsed().as_nanos());
        let timer = std::time::Instant::now();
        let r = self.layout().request_size(info);
        println!("Component::request_size took: {:?}", timer.elapsed().as_nanos());
        RequestTree(r, requests)
    }

    fn build(&self, size: Size, request: RequestTree) -> SizedTree {
        let size = request.0.get(size);
        let children = request.1.iter().map(|b| b.0).collect::<Vec<_>>();
        SizedTree(
            size,
            self.layout().build(size, children).into_iter()
            .zip(self.children()).zip(request.1)
            .map(|((Area{offset, size}, child), branch)| {
                (offset, child.build(size, branch))
            }).collect()
        )
    }

    fn draw(&self, sized: &SizedTree, poffset: Offset, bound: Rect) -> Vec<(CanvasArea, CanvasItem)> {
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
