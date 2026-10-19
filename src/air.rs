pub use air::{Contract, Id, Metadata, Name};
use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::fmt::Debug;
use std::ops::Deref;

use crate::{Application, traits};

pub type Update<R> = traits::Update<Pending<R>, R>;

pub struct Air(Name, HashMap<TypeId, Box<dyn Any + Send + Sync>>);
impl Air {
    pub fn new<A: Application, T: traits::Air>(air: T) -> Self {
        Self(air.me(), A::contracts(Contracts(air, HashMap::new())).1)
    }
    pub fn me(&self) -> Name {self.0}
    pub fn instances<C: Contract>(&self) -> Instances<C> {
        self.1.get(&TypeId::of::<C>()).unwrap_or_else(|| {
            panic!("Contract {} must be registered first", std::any::type_name::<C>())
        }).downcast_ref::<Instances<C>>().unwrap().clone()
    }
}

pub struct Contracts<A: traits::Air>(A, HashMap<TypeId, Box<dyn Any + Send + Sync>>);
impl<A: traits::Air> Contracts<A> {
    pub fn add<C: Contract>(mut self) -> Self {
        self.1.insert(TypeId::of::<C>(), Box::new(Instances(Box::new(self.0.instances::<C>()))));
        self
    }
}

#[derive(Clone, Debug)]
pub struct Instances<C: Contract>(Box<dyn traits::DynInstances<C>>);
impl<C: Contract> Instances<C> {
    pub fn create(&self, init: C::Init) -> Instance<C> {Instance(self.0.create(init))}
    pub fn list(&self) -> HashMap<Id, Instance<C>> {
        self.0.list().into_iter().map(|(id, instance)| (id, Instance(instance))).collect()
    }
    pub async fn next(&mut self) -> Instance<C> {Instance(self.0.next().await)}
    pub fn try_next(&mut self) -> Option<Instance<C>> {self.0.try_next().map(Instance)}
}

#[derive(Clone, Debug)]
pub struct Instance<C: Contract>(Box<dyn traits::DynInstance<C>>);
impl<C: Contract> Instance<C> {
    pub fn send(&mut self, msg: C::Message) -> Pending<C::Result> {Pending(self.0.send(msg))}
    pub fn share(&self, name: Name) {self.0.share(name)}
    pub fn pending(&self) -> Ref<'_, C> {Ref(self.0.pending())}
    pub fn confirmed(&self) -> Option<Ref<'_, C>> {self.0.confirmed().map(Ref)}
    pub async fn next(&mut self) -> Update<C::Result> {
        match self.0.next().await {
            traits::Update::Init => Update::Init,
            traits::Update::Pending(pending) => Update::Pending(Pending(pending)),
            traits::Update::Confirmed(result) => Update::Confirmed(result),
        }
    }
    pub fn try_next(&mut self) -> Option<Update<C::Result>> {
        self.0.try_next().map(|update| match update {
            traits::Update::Init => Update::Init,
            traits::Update::Pending(pending) => Update::Pending(Pending(pending)),
            traits::Update::Confirmed(result) => Update::Confirmed(result),
        })
    }
}

#[derive(Clone, Debug)]
pub struct Pending<R: Debug + Clone + Send + Sync + 'static>(Box<dyn traits::DynPending<R>>);
impl<R: Debug + Clone + Send + Sync + 'static> Pending<R> {
    pub fn pending(&self) -> Ref<'_, R> {Ref(self.0.pending())}
    pub fn try_confirmed(&self) -> Option<R> {self.0.try_confirmed()}
    pub async fn confirmed(self) -> R {self.0.confirmed().await}
    pub fn try_next(&mut self) -> Option<bool> {self.0.try_next()}
    pub async fn next(&mut self) -> bool {self.0.next().await}
}

#[derive(Debug)]
pub struct Ref<'a, R>(Box<dyn traits::Ref<'a, R> + 'a>);
impl<R> Deref for Ref<'_, R> {
    type Target = R;
    fn deref(&self) -> &R {&self.0}
}
