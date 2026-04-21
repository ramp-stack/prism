
pub struct Handler(VecDeq<Box<dyn Event>>, bool);

pub trait Handler {
    fn me(&mut self) -> Name;

    fn create(&mut self, c_id: Id, hash: Id, bytes: Vec<u8>) -> Id;
    fn share(&mut self, c_id: Id, id: Id, name: Name); 
    fn send(&mut self, c_id: Id, id: Id, path: PathBuf, index: usize, bytes: Vec<u8>);
    fn get(&mut self, c_id: Id, id: Id, path: PathBuf) -> Option<Substance>;

    fn emit(&mut self, event: Box<dyn Event>);

    fn start_camera(&mut self);
    fn stop_camera(&mut self);
    fn pick_photo(&mut self);

    fn get_safe_area(&mut self) -> (f32, f32, f32, f32);
    fn share_social(&mut self, data: String);

    fn set_clipboard(&mut self, data: String);
    fn get_clipboard(&mut self) -> String;

    fn trigger_haptic(&mut self);
}
