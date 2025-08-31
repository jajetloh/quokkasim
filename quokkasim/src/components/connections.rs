use serde::Serialize;
use std::fmt::Debug;

use crate::{
    common::ToLogRecord, components::{continuous_traits::{ContinuousResource, Stock}, environment::BasicEnvironment}, nexosim::{Address, Model}, prelude::{
        ContinuousStockLogType, ContinuousStockState, DefaultProcess, DefaultProcessLogType, DefaultStock, Process, Projectable
    }
};

pub trait Connect<A: Model, B: Model> {
    fn connect(&mut self, a: (&mut A, &Address<A>), b: (&mut B, &Address<B>))
    -> Result<(), String>;
}

pub struct Connection;

impl<T: ContinuousResource + 'static, R: Clone + Send + Debug + Serialize + 'static, StockLogRecord: Clone + Send + Debug + Serialize + 'static>
    Connect<DefaultProcess<T, R>, DefaultStock<T, ContinuousStockState, StockLogRecord>> for Connection
where
    DefaultStock<T, ContinuousStockState, StockLogRecord>: Model,
    StockLogRecord: Serialize,
    DefaultStock<T, ContinuousStockState, StockLogRecord>: ToLogRecord<ContinuousStockLogType<T>, StockLogRecord>,
    DefaultProcess<T, R>: Process<T, R, DefaultProcessLogType<T>>,
{
    fn connect(
        &mut self,
        a: (&mut DefaultProcess<T, R>, &Address<DefaultProcess<T, R>>),
        b: (
            &mut DefaultStock<T, ContinuousStockState, StockLogRecord>,
            &Address<DefaultStock<T, ContinuousStockState, StockLogRecord>>,
        ),
    ) -> Result<(), String> {
        a.0.push_downstream.connect(DefaultStock::add, b.1.clone());
        a.0.req_downstream
            .connect(DefaultStock::get_state_async, b.1.clone());
        b.0.state_emitter.connect(DefaultProcess::update_state, a.1);
        Ok(())
    }
}

impl<
    T: ContinuousResource + Projectable<f64> + 'static,
    R: Clone + Send + Debug + Serialize + 'static,
    StockLogRecord: Clone + Send + Debug + Serialize + 'static,
> Connect<DefaultStock<T, ContinuousStockState, StockLogRecord>, DefaultProcess<T, R>> for Connection
where
    DefaultStock<T, ContinuousStockState, StockLogRecord>: Model,
    StockLogRecord: Serialize,
    DefaultStock<T, ContinuousStockState, StockLogRecord>: ToLogRecord<ContinuousStockLogType<T>, StockLogRecord>,
    DefaultProcess<T, R>: Process<T, R, DefaultProcessLogType<T>>,
{
    fn connect(
        &mut self,
        a: (
            &mut DefaultStock<T, ContinuousStockState, StockLogRecord>,
            &Address<DefaultStock<T, ContinuousStockState, StockLogRecord>>,
        ),
        b: (&mut DefaultProcess<T, R>, &Address<DefaultProcess<T, R>>),
    ) -> Result<(), String> {
        b.0.withdraw_upstream
            .connect(DefaultStock::remove, a.1.clone());
        b.0.req_upstream
            .connect(DefaultStock::get_state_async, a.1.clone());
        a.0.state_emitter.connect(DefaultProcess::update_state, b.1);
        Ok(())
    }
}

impl<T: ContinuousResource + 'static, R: Clone + Send + Debug + Serialize + 'static>
    Connect<BasicEnvironment, DefaultProcess<T, R>> for Connection
where
    DefaultProcess<T, R>: Process<T, R, DefaultProcessLogType<T>>,
    BasicEnvironment: Model,
{
    fn connect(
        &mut self,
        a: (&mut BasicEnvironment, &Address<BasicEnvironment>),
        b: (&mut DefaultProcess<T, R>, &Address<DefaultProcess<T, R>>),
    ) -> Result<(), String> {
        a.0.emit_change
            .connect(DefaultProcess::update_state, b.1.clone());
        b.0.req_environment
            .connect(BasicEnvironment::get_state_async, a.1.clone());
        Ok(())
    }
}
