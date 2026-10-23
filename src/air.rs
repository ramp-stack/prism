pub use air::{Contract, Id, Metadata, Name};
use air::{Identity, Location, Output, Response};

use std::any::Any;
use std::collections::HashMap;
use std::fmt::{Debug, Formatter, Result as FmtResult};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::Deref;
use std::sync::Arc;

use crate::events::ChangeSource;
use crate::service::{Key, Service, ServiceInit};
use crate::traits;

pub type Update<R> = traits::Update<Pending<R>, R>;
pub type Request = Box<dyn Any + Send>;
pub type Value = Arc<dyn Any + Send + Sync>;

#[derive(Clone, Copy, Debug)]
pub struct ContractType {
    pub id: Id,
    pub location: fn(&air::Secret, &Request) -> air::Location,
    pub create: fn(air::Secret, air::Location, Option<Request>) -> Box<dyn traits::DynContract>,
    pub restore: for<'de> fn(&mut dyn erased_serde::Deserializer<'de>) -> Box<dyn traits::DynContract>,
}
impl ContractType {
    pub fn new<C: Contract>() -> Self {
        Self {
            id: C::id(),
            location: |secret, init| air::Instance::<C>::generate_location(
                secret, init.downcast_ref::<C::Init>().unwrap(),
            ).expect("Could not create the Air instance location"),
            create: |secret, location, init| {
                let instance: air::Instance<C> = match init {
                    Some(init) => air::Instance::new(secret, *init.downcast::<C::Init>().unwrap()),
                    None => air::Instance::receive(secret, location),
                }.expect("Could not open the Air instance");
                Box::new(ContractInstance(instance))
            },
            restore: |deserializer| Box::new(ContractInstance::<C>(
                erased_serde::deserialize(deserializer).expect("Invalid cached Air instance"),
            )),
        }
    }
}

pub struct Ref<'a, T>(Box<dyn traits::DynRef + 'a>, PhantomData<T>);
impl<'a, T> Ref<'a, T> {
    pub fn new(value: Box<dyn traits::DynRef + 'a>) -> Self {Self(value, PhantomData)}
}
impl<T: 'static> Deref for Ref<'_, T> {
    type Target = T;
    fn deref(&self) -> &T {self.0.value().downcast_ref().unwrap()}
}
impl<T: Debug + 'static> Debug for Ref<'_, T> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {(**self).fmt(f)}
}

#[derive(Clone)]
pub struct Air(Box<dyn traits::DynAir>);
impl Air {
    pub fn new<T: traits::Air>(air: T) -> Self {Self(Box::new(air))}
    pub fn me(&self) -> Name {self.0.me()}
    pub fn instances<C: Contract>(&self) -> Instances<C> {Instances::new(self)}
    pub fn start<S: Service>(&self, init: S::Init) {
        let air = self.clone();
        let init = ServiceInit {
            key: Key::new::<S>(self.me(), &init),
            service: Box::pin(async move {Box::new(S::init(air, init).await) as Box<dyn traits::DynService>}),
        };
        self.0.start(init);
    }
    pub fn shutdown<S: Service>(&self, init: &S::Init) {
        self.0.shutdown(Key::new::<S>(self.me(), init));
    }
}
impl Debug for Air {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {f.debug_tuple("Air").field(&self.me()).finish()}
}
impl Hash for Air {
    fn hash<H: Hasher>(&self, hasher: &mut H) {self.me().hash(hasher)}
}

#[derive(Clone)]
pub struct Instances<C: Contract>(Box<dyn traits::DynInstances>, PhantomData<C>);
impl<C: Contract> Instances<C> {
    pub fn new(air: &Air) -> Self {Self(air.0.instances(ContractType::new::<C>()), PhantomData)}
    pub fn create(&self, init: C::Init) -> Instance<C> {Instance(self.0.create(Box::new(init)), PhantomData)}
    pub fn list(&self) -> HashMap<Id, Instance<C>> {
        self.0.list().into_iter().map(|(id, instance)| (id, Instance(instance, PhantomData))).collect()
    }
    pub fn try_next(&mut self) -> Option<Instance<C>> {
        self.0.try_next().map(|instance| Instance(instance, PhantomData))
    }
    pub async fn next(&mut self) -> Instance<C> {Instance(self.0.next().await, PhantomData)}
}
impl<C: Contract> Debug for Instances<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {self.0.fmt(f)}
}

impl<C: Contract> ChangeSource for Instances<C> {
    type Update = Instance<C>;
    async fn next(&mut self) -> Option<Self::Update> {Some(Instances::next(self).await)}
}

#[derive(Clone)]
pub struct Instance<C: Contract>(Box<dyn traits::DynInstance>, PhantomData<C>);
impl<C: Contract> Instance<C> {
    pub fn id(&self) -> Id {self.0.id()}
    pub fn notify(&self, enabled: bool) {self.0.notify(enabled)}
    pub fn send(&mut self, message: C::Message) -> Pending<C::Result> {
        Pending(self.0.send(Box::new(message)), PhantomData)
    }
    pub fn share(&self, name: Name) {self.0.share(name)}
    pub fn pending(&self) -> Ref<'_, C> {Ref::new(self.0.pending())}
    pub fn confirmed(&self) -> Option<Ref<'_, C>> {self.0.confirmed().map(Ref::new)}
    pub fn try_next(&mut self) -> Option<Update<C::Result>> {self.0.try_next().map(Self::update)}
    pub async fn next(&mut self) -> Update<C::Result> {Self::update(self.0.next().await)}
    fn update(update: traits::ContractUpdate) -> Update<C::Result> {
        match update {
            traits::Update::Init => Update::Init,
            traits::Update::Pending(pending) => Update::Pending(Pending(pending, PhantomData)),
            traits::Update::Confirmed(value) => {
                Update::Confirmed(value.downcast_ref::<C::Result>().unwrap().clone())
            }
        }
    }
}
impl<C: Contract> Debug for Instance<C> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {self.0.fmt(f)}
}
impl<C: Contract> Hash for Instance<C> {
    fn hash<H: Hasher>(&self, hasher: &mut H) {self.id().hash(hasher)}
}

impl<C: Contract> ChangeSource for Instance<C> {
    type Update = Update<C::Result>;
    async fn next(&mut self) -> Option<Self::Update> {Some(Instance::next(self).await)}
}

#[derive(Clone)]
pub struct Pending<R: Clone + Debug + Send + Sync + 'static>(Box<dyn traits::DynPending>, PhantomData<R>);
impl<R: Clone + Debug + Send + Sync + 'static> Pending<R> {
    pub fn pending(&self) -> Ref<'_, R> {Ref::new(self.0.pending())}
    pub fn try_confirmed(&self) -> Option<R> {
        self.0.try_confirmed().map(|value| value.downcast_ref::<R>().unwrap().clone())
    }
    pub async fn confirmed(mut self) -> R {
        loop {
            if let Some(result) = self.try_confirmed() {return result}
            self.next().await;
        }
    }
    pub fn try_next(&mut self) -> Option<bool> {self.0.try_next()}
    pub async fn next(&mut self) -> bool {self.0.next().await}
}
impl<R: Clone + Debug + Send + Sync + 'static> Debug for Pending<R> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {self.0.fmt(f)}
}

impl<R: Clone + Debug + Send + Sync + 'static> ChangeSource for Pending<R> {
    type Update = bool;
    async fn next(&mut self) -> Option<Self::Update> {Some(Pending::next(self).await)}
}

#[derive(Clone, Debug, serde::Serialize)]
#[serde(transparent)]
struct ContractInstance<C: Contract>(air::Instance<C>);
impl<C: Contract> traits::DynContract for ContractInstance<C> {
    fn id(&self) -> Id {self.0.id()}
    fn location(&self) -> Location {*self.0.location()}
    fn subscription(&self) -> Option<(air::PublicKey, Name)> {self.0.subscription()}
    fn pending(&self) -> Option<&(dyn Any + Send + Sync)> {
        self.0.pending().map(|value| value as &(dyn Any + Send + Sync))
    }
    fn confirmed(&self) -> Option<&(dyn Any + Send + Sync)> {
        self.0.confirmed().map(|value| value as &(dyn Any + Send + Sync))
    }
    fn send(&mut self, message: Request) -> Value {
        Arc::new(self.0.send(*message.downcast::<C::Message>().unwrap()))
    }
    fn start(&mut self) -> air::Request {self.0.start()}
    fn request(&mut self) -> Option<air::Request> {self.0.request()}
    fn response<'a>(&'a mut self, resolver: &'a dyn traits::Resolver, response: Response) -> traits::BoxFuture<'a, Output<Value>> {
        Box::pin(async move {
            let value = |result| Arc::new(result) as Value;
            match self.0.response(&mut Resolver(resolver), response).await {
                Output::Init(results) => Output::Init(results.into_iter().map(value).collect()),
                Output::Created(result, results) => {
                    Output::Created(value(result), results.into_iter().map(value).collect())
                }
                Output::Read(result, results) => {
                    Output::Read(value(result), results.into_iter().map(value).collect())
                }
                Output::Subscribed => Output::Subscribed,
                Output::Garbage => Output::Garbage,
            }
        })
    }
}

struct Resolver<'a>(&'a dyn traits::Resolver);
impl air::Resolver for Resolver<'_> {
    async fn resolve(&mut self, name: Name, timestamp: Option<u64>) -> Arc<Identity> {
        self.0.resolve(name, timestamp).await
    }
}
