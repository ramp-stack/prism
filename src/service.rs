use std::any::TypeId;
use std::hash::{DefaultHasher, Hash, Hasher};
use crate::air::{Air, Name};
use crate::traits;

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub struct Key(pub TypeId, pub u64);
impl Key {
    pub fn new<S: Service>(name: Name, init: &S::Init) -> Self {
        let mut hash = DefaultHasher::new();
        name.hash(&mut hash);
        init.hash(&mut hash);
        Self(TypeId::of::<S>(), hash.finish())
    }
}

pub struct ServiceInit {
    pub key: Key,
    pub service: traits::BoxFuture<'static, Box<dyn traits::DynService>>,
}

pub trait Service: Send + Sized + 'static {
    type Init: Hash + Send + 'static;
    type Predicate: Send;

    fn init(air: Air, init: Self::Init) -> impl Future<Output = Self> + Send;
    fn predicate(&mut self) -> impl Future<Output = Self::Predicate> + Send;
    fn handle(&mut self, predicate: Self::Predicate) -> impl Future<Output = ()> + Send;
}
