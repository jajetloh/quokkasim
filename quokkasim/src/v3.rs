use nexosim::model::{Context, Model};

pub struct EventId {}

pub trait Process<T: Resource> {
    fn update_state(
        &mut self, mut source_event_id: EventId, cx: &mut Context<Self>
    ) -> impl Future<Output = ()> + Send where Self: Model {
        async move {}
    }
}

pub trait Projectable<T: Resource> {
    fn project(self) -> T;
}

pub trait StockState {}
pub struct ExampleStockState {}
impl StockState for ExampleStockState {}

pub trait Stock<T: Resource, S: StockState> {
    fn get_state_async(&self) -> impl Future<Output = S> + Send {
        async move { unimplemented!() }
    }
    fn add(&mut self, item: T) -> impl Future<Output = ()> + Send {
        async move {}
    }
    fn remove(&mut self, item: impl Projectable<T>) -> impl Future<Output = T> + Send {
        async move { unimplemented!() }
    }
}

// trait ContinuousResource {}
// trait DiscreteResource {}
pub trait Resource {}

// ------ //

pub struct IronOre {}
impl Resource for IronOre {}

pub struct Energy {}
impl Resource for Energy {}

impl Resource for f64 {}

// ------ //

pub struct DefaultProcess<T: Resource> {
    pub t: T,
}
impl<T: Resource> Process<T> for DefaultProcess<T> {}

pub struct DefaultStock<T: Resource, S: StockState> {
    pub t: T,
    pub s: S,
}
impl<T: Resource, S: StockState> Stock<T, S> for DefaultStock<T, S> {
    fn get_state_async(&self) -> impl Future<Output = S> + Send {
        async move { unimplemented!() }
    }

    fn add(&mut self, _item: T) -> impl Future<Output = ()> + Send {
        async move {}
    }

    fn remove(&mut self, _item: impl Projectable<T>) -> impl Future<Output = T> + Send {
        async move { unimplemented!() }
    }
}

// ------ //

pub trait Connect<A, B> {
    fn connect(&mut self, a: &mut A, b: &mut B) -> Result<(), String>;
}

pub struct Connection;

impl<T: Resource, U: StockState> Connect<DefaultProcess<T>, DefaultStock<T, U>> for Connection {
    fn connect(&mut self, a: &mut DefaultProcess<T>, b: &mut DefaultStock<T, U>) -> Result<(), String> {
        // Example connection logic
        Ok(())
    }
}