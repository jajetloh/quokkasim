use serde::Serialize;
use std::{error::Error, fmt::Debug};

use crate::{
    components::{continuous_traits::{ContResource, ContStock}, mixed::DefaultLoadingProcess}, nexosim::{Address, Model}, prelude::*,
};

use super::mixed::DefaultUnloadingProcess;

pub trait Connect<A: Model, B: Model> {
    fn connect(&mut self, a: (&mut A, &Address<A>, Option<usize>), b: (&mut B, &Address<B>, Option<usize>))
    -> Result<(), Box<dyn Error>>;
}

pub struct Connection;

impl Default for Connection {
    fn default() -> Self {
        Self::new()
    }
}

impl Connection {
    pub fn new() -> Self {
        Connection
    }
}

// ──────────────────────────── DefaultProcess ────────────────────────────
/* #region DefaultProcess */

impl<
    ResourceType: ContResource + 'static
> Connect<DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>, DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>> for Connection
where
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: ContStock<ResourceType, ContStockState>,
    DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>: ContProcessCore,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>>, Option<usize>),
        b: (
            &mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
            &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>,
            Option<usize>
        ),
    ) -> Result<(), Box<dyn Error>> {
        a.0.req_downstream.connect(DefaultContStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
        b.0.state_emitter.connect(DefaultContProcess::update_state, a.1);
        Ok(())
    }
}

impl<
    ResourceType: ContResource + Projectable<f64> + 'static,
> Connect<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>> for Connection
where
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: Model,
    DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>: Model
{
    fn connect(
        &mut self,
        a: (
            &mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
            &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>,
            Option<usize>
        ),
        b: (&mut DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.withdraw_upstream.connect(DefaultContStock::remove, a.1.clone());
        b.0.req_upstream.connect(DefaultContStock::get_state_async, a.1.clone());
        a.0.state_emitter.connect(DefaultContProcess::update_state, b.1);
        Ok(())
    }
}

/* #endregion DefaultProcess */

// ──────────────────────────── DefaultContSink ────────────────────────────
/* #region DefaultContSink */

impl<
    ResourceType: ContResource + 'static,
>
    Connect<
        DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
        DefaultContSink<ResourceType, ContProcessLog<ResourceType>>,
    > for Connection
where
    DefaultContSink<ResourceType, ContProcessLog<ResourceType>>: Model,
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: Model,
    ResourceType: Projectable<f64>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>, Option<usize>),
        b: (&mut DefaultContSink<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContSink<ResourceType, ContProcessLog<ResourceType>>>, Option<usize>)
    ) -> Result<(), Box<dyn Error>> {
        a.0.state_emitter.connect(DefaultContSink::update_state, b.1.clone());
        b.0.req_upstream.connect(DefaultContStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream.connect(DefaultContStock::remove, a.1.clone());
        Ok(())
    }
}

/* #endregion DefaultContSink */


// ──────────────────────────── DefaultContSource ────────────────────────────
/* #region DefaultContSource */

impl<
    ResourceType: ContResource + 'static,
>
    Connect<
        DefaultContSource<ResourceType, ContProcessLog<ResourceType>>,
        DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
    > for Connection
where
    DefaultContSource<ResourceType, ContProcessLog<ResourceType>>: Model,
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: Model,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContSource<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContSource<ResourceType, ContProcessLog<ResourceType>>>, Option<usize>),
        b: (&mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.state_emitter.connect(DefaultContSource::update_state, a.1.clone());
        a.0.req_downstream.connect(DefaultContStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
        Ok(())
    }
}

/* #endregion DefaultContSource */


// ──────────────────────────── DefaultDiscSource ────────────────────────────
/* #region DefaultDiscSource */

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
>
    Connect<
        DefaultDiscSource<ItemType, DiscProcessLog<ItemType>>,
        DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>,
    > for Connection
where
    DiscStockLogType<ItemType>: Serialize,
{
    fn connect(
        &mut self,
        a: (&mut DefaultDiscSource<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscSource<ItemType, DiscProcessLog<ItemType>>>, Option<usize>),
        b: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.state_emitter.connect(DefaultDiscSource::update_state, a.1.clone());
        a.0.req_downstream.connect(DefaultDiscStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultDiscStock::add_multi, b.1.clone());
        Ok(())
    }
}

/* #endregion DefaultDiscSource */


// ──────────────────────────── DefaultDiscProcess ────────────────────────────
/* #region DefaultDiscProcess */

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
>
    Connect<
        DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>,
        DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>,
    > for Connection
where
    DiscStockLogType<ItemType>: Serialize,
{
    fn connect(
        &mut self,
        a: (&mut DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>>, Option<usize>),
        b: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.state_emitter.connect(DefaultDiscProcess::update_state, a.1.clone());
        a.0.req_downstream.connect(DefaultDiscStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultDiscStock::add_multi, b.1.clone());
        Ok(())
    }
}

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
>
    Connect<
        DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>,
        DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>,
    > for Connection
where
    DiscStockLogType<ItemType>: Serialize,
{
    fn connect(
        &mut self,
        a: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>, Option<usize>),
        b: (&mut DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.state_emitter.connect(DefaultDiscProcess::update_state, b.1.clone());
        b.0.req_upstream.connect(DefaultDiscStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream.connect(DefaultDiscStock::remove_multi, a.1.clone());
        Ok(())
    }
}

/* #endregion DefaultDiscProcess */



// ──────────────────────────── DefaultDiscSink ────────────────────────────
/* #region DefaultDiscSink */

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
>
    Connect<
        DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>,
        DefaultDiscSink<ItemType, DiscProcessLog<ItemType>>,
    > for Connection
where
    DiscStockLogType<ItemType>: Serialize,
{
    fn connect(
        &mut self,
        a: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>, Option<usize>),
        b: (&mut DefaultDiscSink<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscSink<ItemType, DiscProcessLog<ItemType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.state_emitter.connect(DefaultDiscSink::update_state, b.1.clone());
        b.0.req_upstream.connect(DefaultDiscStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream.connect(DefaultDiscStock::remove_multi, a.1.clone());
        Ok(())
    }
}

/* #endregion DefaultDiscSink */

// ──────────────────────────── DefaultLoadingProcess ────────────────────────────
/* #region DefaultLoadingProcess */
impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
> for Connection
where
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: Model,
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: DiscProcessCore<ContainerType, ContainerProcessLogType>,
    ResourceType: Projectable<f64>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>, Option<usize>),
        b: (&mut DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.req_upstream_resources.connect(DefaultContStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream_resources.connect(DefaultContStock::remove, a.1.clone());
        a.0.state_emitter.connect(DefaultLoadingProcess::update_state, b.1.clone());
        Ok(())
    }
}

impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>,
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
> for Connection
where
    DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>: Model,
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: DiscProcessCore<ContainerType, ContainerProcessLogType>,
    ResourceType: Projectable<f64>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>, &Address<DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>>, Option<usize>),
        b: (&mut DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.req_upstream_vehicles.connect(DefaultDiscStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream_vehicles.connect(DefaultDiscStock::remove_multi, a.1.clone());
        a.0.state_emitter.connect(DefaultLoadingProcess::update_state, b.1.clone());
        Ok(())
    }
}

impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
    DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>,
> for Connection
where
    DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>: Model,
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: DiscProcessCore<ContainerType, ContainerProcessLogType>,
    ResourceType: Projectable<f64>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
        b: (&mut DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>, &Address<DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.req_downstream.connect(DefaultDiscStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultDiscStock::add_multi, b.1.clone());
        b.0.state_emitter.connect(DefaultLoadingProcess::update_state, a.1.clone());
        Ok(())
    }
}

impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
    DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>,
> for Connection
where
    DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>: Model,
    DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: Model,
    ResourceType: Projectable<f64>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultLoadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
        b: (&mut DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>, &Address<DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.req_downstream.connect(DefaultTravelProcess::get_state_async, b.1.clone());
        a.0.push_downstream.map_connect(|x| {
            let payload = (x.0.first().unwrap().clone(), x.1.clone());
            payload
        }, DefaultTravelProcess::add, b.1.clone());
        Ok(())
    }
}

/* #endregion DefaultLoadingProcess */

// ──────────────────────────── DefaultUnloadingProcess ────────────────────────────
/* #region DefaultUnloadingProcess */


impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>,
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
> for Connection
where
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: DiscProcessCore<ContainerType, ContainerProcessLogType>,
{
    fn connect(&mut self, 
        a: (&mut DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>, &Address<DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>>, Option<usize>),
        b: (&mut DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        b.0.req_upstream.connect(DefaultDiscStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream.connect(DefaultDiscStock::remove_multi, a.1.clone());
        a.0.state_emitter.connect(DefaultUnloadingProcess::update_state, b.1.clone());
        Ok(())
    }
}

impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
> for Connection
where
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: DiscProcessCore<ContainerType, ContainerProcessLogType>,
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: Model,
{
    fn connect(&mut self, 
        a: (&mut DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
        b: (&mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.req_downstream_resources.connect(DefaultContStock::get_state_async, b.1.clone());
        a.0.push_downstream_resources.connect(DefaultContStock::add, b.1.clone());
        b.0.state_emitter.connect(DefaultUnloadingProcess::update_state, a.1.clone());
        Ok(())
    }
}

impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
    DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>,
> for Connection
where
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: DiscProcessCore<ContainerType, ContainerProcessLogType>,
{
    fn connect(&mut self, 
        a: (&mut DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
        b: (&mut DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>, &Address<DefaultDiscStock<ContainerType, DiscStockState, DiscStockLog<ContainerType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.req_downstream_vehicles.connect(DefaultDiscStock::get_state_async, b.1.clone());
        a.0.push_downstream_vehicles.connect(DefaultDiscStock::add_multi, b.1.clone());
        b.0.state_emitter.connect(DefaultUnloadingProcess::update_state, a.1.clone());
        Ok(())
    }
}

impl<
    ContainerType: Debug + Clone + Send + Serialize + 'static,
    ContainerProcessLogType: Clone + Send + 'static,
    ResourceType: Projectable<f64> + ContArithmetic + Default + Debug + Clone + Serialize + Send + 'static,
> Connect<
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>,
    DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>,
> for Connection
where
    DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>: Model,
    DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>: Model,
{
    fn connect(
        &mut self,
        a: (&mut DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>, &Address<DefaultUnloadingProcess<ContainerType, ContainerProcessLogType, ResourceType>>, Option<usize>),
        b: (&mut DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>, &Address<DefaultTravelProcess<ContainerType, DiscProcessLog<ContainerType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.push_downstream_vehicles.map_connect(|x| { (x.0.first().unwrap().clone(), x.1.clone()) }, DefaultTravelProcess::add, b.1.clone());
        a.0.req_downstream_vehicles.connect(DefaultTravelProcess::get_state_async, b.1.clone());
        Ok(())
    }
}

/* #endregion DefaultUnloadingProcess */

// ──────────────────────────── DefaultTravelProcess ────────────────────────────
/* #region DefaultTravelProcess */

impl<
    ItemType: Clone + Debug + Serialize + Send + 'static,
> Connect<
    DefaultTravelProcess<ItemType, DiscProcessLog<ItemType>>,
    DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>,
> for Connection
where
    DiscStockLogType<ItemType>: Serialize,
{
    fn connect(
        &mut self,
        a: (&mut DefaultTravelProcess<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultTravelProcess<ItemType, DiscProcessLog<ItemType>>>, Option<usize>),
        b: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>, Option<usize>),
    ) -> Result<(), Box<dyn Error>> {
        a.0.req_destination.insert(b.0.element_code.clone(), Requestor::new());
        a.0.req_destination.get_mut(&b.0.element_code).unwrap().connect(DefaultDiscStock::get_state_async, b.1.clone());
        a.0.push_to_destination.insert(b.0.element_code.clone(), Output::new());
        a.0.push_to_destination.get_mut(&b.0.element_code).unwrap().map_connect(|x| (Some(x.0.clone()), x.1.clone()), DefaultDiscStock::add_one, b.1.clone());
        Ok(())
    }
}


/* #endregion DefaultTravelProcess */