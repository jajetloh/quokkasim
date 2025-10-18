use serde::Serialize;
use std::fmt::Debug;

use crate::{
    components::{continuous_traits::{ContResource, ContStock}, environment::BasicEnvironment}, nexosim::{Address, Model}, prelude::*,
};

pub trait Connect<A: Model, B: Model> {
    fn connect(&mut self, a: (&mut A, &Address<A>), b: (&mut B, &Address<B>))
    -> Result<(), String>;
}

pub struct Connection;

// ──────────────────────────── DefaultProcess ────────────────────────────
/* #region DefaultProcess */

impl<
    ResourceType: ContResource + 'static
> Connect<DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>, DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>> for Connection
where
    DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>: ContStock<ResourceType, ContStockState>,
    DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>: ContProcessCore<ResourceType, ContProcessLog<ResourceType>>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>>),
        b: (
            &mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>,
            &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>,
        ),
    ) -> Result<(), String> {
        a.0.req_downstream.connect(DefaultContStock::get_state_async, b.1.clone());
        a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
        b.0.state_emitter.connect(DefaultContProcess::update_state, a.1);
        Ok(())
    }
}

// impl<
//     ResourceType: ContResource + 'static,
//     ProcessLogRecord: Clone + Send + Debug + Serialize + 'static,
//     StockLogRecord: StockState + Clone + Send + Debug + Serialize + 'static
// > Connect<DefaultContProcess<ResourceType, ProcessLogRecord>, DefaultContStock<ResourceType, ContStockState, StockLogRecord>> for Connection
// where
//     DefaultContStock<ResourceType, ContStockState, StockLogRecord>: ContStock<ResourceType, StockLogRecord>,
//     DefaultContProcess<ResourceType, ProcessLogRecord>: ContProcessCore<ResourceType, ProcessLogRecord>,
// {
//     fn connect(
//         &mut self,
//         a: (&mut DefaultContProcess<ResourceType, ProcessLogRecord>, &Address<DefaultContProcess<ResourceType, ProcessLogRecord>>),
//         b: (
//             &mut DefaultContStock<ResourceType, ContStockState, StockLogRecord>,
//             &Address<DefaultContStock<ResourceType, ContStockState, StockLogRecord>>,
//         ),
//     ) -> Result<(), String> {
//         a.0.req_downstream.connect(DefaultContStock::get_state_async, b.1.clone());
//         a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
//         b.0.state_emitter.connect(DefaultContProcess::update_state, a.1);
//         Ok(())
//     }
// }

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
        ),
        b: (&mut DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContProcess<ResourceType, ContProcessLog<ResourceType>>>),
    ) -> Result<(), String> {
        b.0.withdraw_upstream.connect(DefaultContStock::remove, a.1.clone());
        b.0.req_upstream.connect(DefaultContStock::get_state_async, a.1.clone());
        a.0.state_emitter.connect(DefaultContProcess::update_state, b.1);
        Ok(())
    }
}


// impl<
//     ResourceType: ContResource + Projectable<f64> + 'static,
//     ProcessLogRecord: Clone + Send + Debug + Serialize + 'static,
//     StockLogRecord: Clone + Send + Debug + Serialize + 'static,
// > Connect<DefaultContStock<ResourceType, ContStockState, StockLogRecord>, DefaultContProcess<ResourceType, ProcessLogRecord>> for Connection
// where
//     DefaultContStock<ResourceType, ContStockState, StockLogRecord>: Model,
//     DefaultContProcess<ResourceType, ProcessLogRecord>: Model
// {
//     fn connect(
//         &mut self,
//         a: (
//             &mut DefaultContStock<ResourceType, ContStockState, StockLogRecord>,
//             &Address<DefaultContStock<ResourceType, ContStockState, StockLogRecord>>,
//         ),
//         b: (&mut DefaultContProcess<ResourceType, ProcessLogRecord>, &Address<DefaultContProcess<ResourceType, ProcessLogRecord>>),
//     ) -> Result<(), String> {
//         b.0.withdraw_upstream
//             .connect(DefaultContStock::remove, a.1.clone());
//         b.0.req_upstream
//             .connect(DefaultContStock::get_state_async, a.1.clone());
//         a.0.state_emitter.connect(DefaultContProcess::update_state, b.1);
//         Ok(())
//     }
// }

/* #endregion DefaultProcess */

// ──────────────────────────── DefaultContSink ────────────────────────────
/* #region DefaultContSink */

impl<
    ResourceType: ContResource + 'static,
    // StockLogRecord: Clone + Send + Debug + Serialize + 'static,
    // ProcessLogRecord: Clone + Send + Debug + Serialize + 'static,
    // TODO: Make more generic, ContinuousProcessLog<...> -> ProcessLogRecord
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
        a: (&mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>),
        b: (&mut DefaultContSink<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContSink<ResourceType, ContProcessLog<ResourceType>>>)
    ) -> Result<(), String> {
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
        a: (&mut DefaultContSource<ResourceType, ContProcessLog<ResourceType>>, &Address<DefaultContSource<ResourceType, ContProcessLog<ResourceType>>>),
        b: (&mut DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>, &Address<DefaultContStock<ResourceType, ContStockState, ContStockLog<ResourceType>>>),
    ) -> Result<(), String> {
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
    // DefaultDiscSource<ItemType, DiscProcessLog<ItemType>>: DiscProcessCore<ItemType, DiscProcessLog<ItemType>>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultDiscSource<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscSource<ItemType, DiscProcessLog<ItemType>>>),
        b: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>),
    ) -> Result<(), String> {
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
        a: (&mut DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>>),
        b: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>),
    ) -> Result<(), String> {
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
        a: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>),
        b: (&mut DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscProcess<ItemType, DiscProcessLog<ItemType>>>),
    ) -> Result<(), String> {
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
        a: (&mut DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>, &Address<DefaultDiscStock<ItemType, DiscStockState, DiscStockLog<ItemType>>>),
        b: (&mut DefaultDiscSink<ItemType, DiscProcessLog<ItemType>>, &Address<DefaultDiscSink<ItemType, DiscProcessLog<ItemType>>>),
    ) -> Result<(), String> {
        a.0.state_emitter.connect(DefaultDiscSink::update_state, b.1.clone());
        b.0.req_upstream.connect(DefaultDiscStock::get_state_async, a.1.clone());
        b.0.withdraw_upstream.connect(DefaultDiscStock::remove_multi, a.1.clone());
        Ok(())
    }
}

/* #endregion DefaultDiscSink */
