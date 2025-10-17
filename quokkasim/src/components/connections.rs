use serde::Serialize;
use std::fmt::Debug;

use crate::{
    common::ToLogRecord, components::{continuous_traits::{ContResource, ContStock}, environment::BasicEnvironment}, nexosim::{Address, Model}, prelude::*,
};

pub trait Connect<A: Model, B: Model> {
    fn connect(&mut self, a: (&mut A, &Address<A>), b: (&mut B, &Address<B>))
    -> Result<(), String>;
}

pub struct Connection;

// ──────────────────────────── DefaultProcess ────────────────────────────
/* #region DefaultProcess */

impl<T: ContResource + 'static, R: Clone + Send + Debug + Serialize + 'static, StockLogRecord: Clone + Send + Debug + Serialize + 'static>
    Connect<DefaultContProcess<T, R>, DefaultContStock<T, ContStockState, StockLogRecord>> for Connection
where
    DefaultContStock<T, ContStockState, StockLogRecord>: Model,
    StockLogRecord: Serialize,
    DefaultContStock<T, ContStockState, StockLogRecord>: ToLogRecord<ContStockLogType<T>, StockLogRecord>,
    DefaultContProcess<T, R>: ContProcess<T, R, DefaultContProcessLogType<T>>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContProcess<T, R>, &Address<DefaultContProcess<T, R>>),
        b: (
            &mut DefaultContStock<T, ContStockState, StockLogRecord>,
            &Address<DefaultContStock<T, ContStockState, StockLogRecord>>,
        ),
    ) -> Result<(), String> {
        a.0.push_downstream.connect(DefaultContStock::add, b.1.clone());
        a.0.req_downstream
            .connect(DefaultContStock::get_state_async, b.1.clone());
        b.0.state_emitter.connect(DefaultContProcess::update_state, a.1);
        Ok(())
    }
}

impl<
    T: ContResource + Projectable<f64> + 'static,
    R: Clone + Send + Debug + Serialize + 'static,
    StockLogRecord: Clone + Send + Debug + Serialize + 'static,
> Connect<DefaultContStock<T, ContStockState, StockLogRecord>, DefaultContProcess<T, R>> for Connection
where
    DefaultContStock<T, ContStockState, StockLogRecord>: Model,
    StockLogRecord: Serialize,
    DefaultContStock<T, ContStockState, StockLogRecord>: ToLogRecord<ContStockLogType<T>, StockLogRecord>,
    DefaultContProcess<T, R>: ContProcess<T, R, DefaultContProcessLogType<T>>,
{
    fn connect(
        &mut self,
        a: (
            &mut DefaultContStock<T, ContStockState, StockLogRecord>,
            &Address<DefaultContStock<T, ContStockState, StockLogRecord>>,
        ),
        b: (&mut DefaultContProcess<T, R>, &Address<DefaultContProcess<T, R>>),
    ) -> Result<(), String> {
        b.0.withdraw_upstream
            .connect(DefaultContStock::remove, a.1.clone());
        b.0.req_upstream
            .connect(DefaultContStock::get_state_async, a.1.clone());
        a.0.state_emitter.connect(DefaultContProcess::update_state, b.1);
        Ok(())
    }
}

impl<T: ContResource + 'static, R: Clone + Send + Debug + Serialize + 'static>
    Connect<BasicEnvironment, DefaultContProcess<T, R>> for Connection
where
    DefaultContProcess<T, R>: ContProcess<T, R, DefaultContProcessLogType<T>>,
    BasicEnvironment: Model,
{
    fn connect(
        &mut self,
        a: (&mut BasicEnvironment, &Address<BasicEnvironment>),
        b: (&mut DefaultContProcess<T, R>, &Address<DefaultContProcess<T, R>>),
    ) -> Result<(), String> {
        a.0.emit_change
            .connect(DefaultContProcess::update_state, b.1.clone());
        b.0.req_environment
            .connect(BasicEnvironment::get_state_async, a.1.clone());
        Ok(())
    }
}

/* #endregion DefaultProcess */

// ──────────────────────────── DefaultContSink ────────────────────────────
/* #region DefaultContSink */

impl<
    ResourceType: ContResource + 'static,
    StockLogRecord: Clone + Send + Debug + Serialize + 'static,
    // ProcessLogRecord: Clone + Send + Debug + Serialize + 'static,
    // TODO: Make more generic, ContinuousProcessLog<...> -> ProcessLogRecord
>
    Connect<
        DefaultContStock<ResourceType, ContStockState, StockLogRecord>,
        DefaultContSink<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>,
    > for Connection
where
    DefaultContSink<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>: Model,
    DefaultContStock<ResourceType, ContStockState, StockLogRecord>: ToLogRecord<ContStockLogType<ResourceType>, StockLogRecord>,
    DefaultContStock<ResourceType, ContStockState, StockLogRecord>: Model,
    StockLogRecord: Serialize,
    ResourceType: Projectable<f64>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContStock<ResourceType, ContStockState, StockLogRecord>, &Address<DefaultContStock<ResourceType, ContStockState, StockLogRecord>>),
        b: (&mut DefaultContSink<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>, &Address<DefaultContSink<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>>)
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
    StockLogRecord: Clone + Send + Debug + Serialize + 'static,
>
    Connect<
        DefaultContSource<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>,
        DefaultContStock<ResourceType, ContStockState, StockLogRecord>,
    > for Connection
where
    DefaultContSource<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>: Model,
    DefaultContStock<ResourceType, ContStockState, StockLogRecord>: ToLogRecord<ContStockLogType<ResourceType>, StockLogRecord>,
    DefaultContStock<ResourceType, ContStockState, StockLogRecord>: Model,
    StockLogRecord: Serialize,
{
    fn connect(
        &mut self,
        a: (&mut DefaultContSource<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>, &Address<DefaultContSource<ResourceType, ContProcessLog<DefaultContProcessLogType<ResourceType>, ResourceType>>>),
        b: (&mut DefaultContStock<ResourceType, ContStockState, StockLogRecord>, &Address<DefaultContStock<ResourceType, ContStockState, StockLogRecord>>),
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
