use downcast_rs::{Downcast, impl_downcast};

use crate::event::Event;

pub trait Request: Downcast {
    type Response where Self: Sized;
}
impl_downcast!(Request);

#[cfg(feature="maverick_os")]
impl Request for maverick_os::Request {
    type Response = <Self as maverick_os::Request>::Response;
}

pub struct StartCamera;
impl Request for StartCamera {type Response = ();}

pub struct StopCamera;
impl Request for StopCamera {type Response = ();}

pub struct GetFrame;
impl Request for GetFrame {type Response = Vec<u8>;}//Image

pub struct GetClipboard;
impl Request for GetClipboard {type Response = String;}

pub struct SetClipboard(pub String);
impl Request for SetClipboard {type Response = ();}

pub struct EmitEvent(pub Box<dyn Event>);
impl Request for EmitEvent {type Response = ();}

//  #[derive(Debug)]
//  pub enum Hardware {
//      GetCamera,
//      StopCamera,
//      GetSafeArea,
//      PhotoPicker,
//      SetClipboard(String),
//      GetClipboard,
//    //SetCloud(String, String),
//    //GetCloud(String),
//      Share(String),
//      Haptic,
//  }
