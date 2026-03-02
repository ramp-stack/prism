use prism::canvas::{Shape, ShapeType, Color, Align, Span, Font};
use prism::{Request, Hardware, self, event, canvas::{Image, Text}, Context, layout::{Offset, Row, Area, Stack, Column, Size, Padding}, event::{OnEvent, Event, TickEvent}, drawable::{Component, SizedTree}, drawables};
use std::sync::Arc;
use image::RgbaImage;
use prism::display::{Bin, EitherOr, Opt, Enum};
use prism::emitters::Button;

#[derive(Debug, Component, Clone)]
pub struct Demo(Stack, Vec<Box<dyn Drawable>>);
impl OnEvent for Demo {}
impl Demo {
    pub fn start(ctx: &mut Context, assets: &Assets) -> Self {
        Self(Stack::new(Offset::Center, Offset::Center, Size::Fill, Size::Fill, Padding::new(24.0)), drawables![Selector::new(assets)])
    }
}

ramp::run!{|ctx: &mut Context, assets: Assets| Demo::start(ctx, &assets)}

#[derive(Debug, Component, Clone)]
pub struct Selector {
    layout: Column,
    title: Text,
    displayed: ImageDisplay,
    buttons: ButtonRow,
    #[skip] the_rest: Vec<ImageDisplay>,
    #[skip] current: usize,
}

impl OnEvent for Selector {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(event) = event.downcast_ref::<Next>() {
            match event {
                Next::Right => {
                    self.current += 1;
                    if self.current >= self.the_rest.len() {self.current = 0;}
                    self.displayed = self.the_rest[self.current].clone();
                }
                Next::Left => {
                    match self.current == 0 {
                        true => self.current = self.the_rest.len() - 1,
                        false => self.current = self.current.saturating_sub(1),
                    }

                    self.displayed = self.the_rest[self.current].clone();
                }
            }

            self.buttons.update(&self.displayed.3);
        }

        vec![event]
    }
}

impl Selector {
    fn new(assets: &Assets) -> Self {
        let images = ["beach", "lake", "marsh", "mountains", "rainforest", "moon", "desert"].into_iter().map(|p| ImageDisplay::new(assets, p)).collect::<Vec<_>>();
        let font = Font::from_bytes(&assets.get_font("font.ttf").unwrap()).unwrap();
        let text = Text::new(vec![Span::new("View destinations".to_string(), 16.0, Some(16.0*1.25), font.into(), Color(255, 255, 255, 255), 0.0)], None, Align::Center, None);
        Selector {
            layout: Column::center(24.0),
            title: text,
            displayed: images[0].clone(),
            buttons: ButtonRow::new(assets),
            the_rest: images,
            current: 0
        }
    }
}

#[derive(Debug, Component, Clone)]
pub struct ButtonRow(Row, Button<IconButton>, Text, Button<IconButton>);
impl OnEvent for ButtonRow {}

impl ButtonRow {
    fn new(assets: &Assets) -> Self {
        let font = Font::from_bytes(&assets.get_font("font.ttf").unwrap()).unwrap();
        ButtonRow(Row::center(48.0), 
            Button::new(IconButton::left(assets)), 
            Text::new(vec![Span::new("The Beach".to_string(), 22.0, Some(22.0*1.25), font.into(), Color(255, 255, 255, 255), 0.0)], None, Align::Center, None),
            Button::new(IconButton::right(assets))
        )
    }

    fn update(&mut self, new: &String) {
        let s = new[..1].to_ascii_uppercase() + &new[1..];
        self.2.spans[0].text = format!("The {s}");
    }
}


#[derive(Debug, Component, Clone)]
pub struct IconButton(Stack, Image, #[skip] bool);
impl OnEvent for IconButton {
    fn on_event(&mut self, ctx: &mut Context, _sized: &SizedTree, event: Box<dyn Event>) -> Vec<Box<dyn Event>> {
        if let Some(event) = event.downcast_ref::<event::Button>() {
            match event {
                event::Button::Pressed(true) => {
                    ctx.send(Request::Hardware(Hardware::Haptic)); 
                    ctx.send(Request::event(match self.2 {
                        true => Next::Left,
                        false => Next::Right
                    }));
                },
                _ => {}
            }
        }

        vec![event]
    }
}

impl IconButton {
    fn left(assets: &Assets) -> Self {
        let img: Arc<RgbaImage> = assets.get_svg("left.svg").unwrap();
        let image = Image{shape: ShapeType::Rectangle(0.0, (48.0, 48.0), 0.0), image: img.clone(), color: Some(Color(255, 255, 255, 255))};
        IconButton(Stack::default(), image, true)
    }


    fn right(assets: &Assets) -> Self {
        let img: Arc<RgbaImage> = assets.get_svg("right.svg").unwrap();
        let image = Image{shape: ShapeType::Rectangle(0.0, (48.0, 48.0), 0.0), image: img.clone(), color: Some(Color(255, 255, 255, 255))};
        IconButton(Stack::default(), image, false)
    }

}

#[derive(Debug, Clone, Component)]
pub struct ImageDisplay(Stack, Shape, Image, #[skip] String);
impl OnEvent for ImageDisplay {}
impl ImageDisplay {
    pub fn new(assets: &Assets, path: &str) -> Self {
        let shape = Shape{shape: ShapeType::RoundedRectangle(0.0, (325.0, 425.0), 0.0, 8.0), color: Color(255, 255, 255, 255)};
        let img: Arc<RgbaImage> = assets.get_image(&format!("{}.png", path)).unwrap();
        let image = Image{shape: ShapeType::RoundedRectangle(0.0, (300.0, 400.0), 0.0, 8.0), image: img.clone(), color: None};
        ImageDisplay(Stack::center(), shape, image, path.to_string())
    }
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Next {Left, Right}

impl Event for Next {
    fn pass(self: Box<Self>, _ctx: &mut Context, children: &[Area]) -> Vec<Option<Box<dyn Event>>> {
        children.iter().map(|_| Some(self.clone() as Box<dyn Event>)).collect()
    }
}
