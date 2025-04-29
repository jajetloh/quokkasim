pub mod process;
pub mod stock;

use process::{DumpingProcess, LoadingProcess, TruckMovementProcess};
use nexosim::simulation::Address;
use quokkasim::{core::Mailbox, prelude::{ArrayResource, ArrayStock}};
use serde::Deserialize;
use stock::TruckStock;

pub enum ComponentModel {
    ArrayStock(ArrayStock, Mailbox<ArrayStock>, Address<ArrayStock>),
    TruckStock(TruckStock, Mailbox<TruckStock>, Address<TruckStock>),
    LoadingProcess(LoadingProcess, Mailbox<LoadingProcess>, Address<LoadingProcess>),
    DumpingProcess(DumpingProcess, Mailbox<DumpingProcess>, Address<DumpingProcess>),
    TruckMovementProcess(TruckMovementProcess, Mailbox<TruckMovementProcess>, Address<TruckMovementProcess>),
}

impl ComponentModel {
    pub fn get_name(&self) -> &String {
        match self {
            ComponentModel::ArrayStock(x, _, _) => &x.element_name,
            ComponentModel::TruckStock(x, _, _) => &x.element_name,
            ComponentModel::LoadingProcess(x, _, _) => &x.element_name,
            ComponentModel::DumpingProcess(x, _, _) => &x.element_name,
            ComponentModel::TruckMovementProcess(x, _, _) => &x.element_name,
        }
    }
}

pub enum ComponentModelAddress {
    ArrayStock(Address<ArrayStock>),
    TruckStock(Address<TruckStock>),
    LoadingProcess(Address<LoadingProcess>),
    DumpingProcess(Address<DumpingProcess>),
    TruckMovementProcess(Address<TruckMovementProcess>),
}


#[derive(Debug, Clone)]
pub struct TruckAndOre {
    pub truck: i32,
    pub ore: ArrayResource,
}