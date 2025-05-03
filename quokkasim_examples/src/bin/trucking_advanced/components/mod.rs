pub mod process;
pub mod stock;

use process::{DumpingProcess, LoadingProcess, TruckMovementProcess};
use nexosim::simulation::Address;
use quokkasim::{core::Mailbox, prelude::{VectorResource, VectorStock}};
use stock::TruckStock;

pub enum ComponentModel {
    VectorStock(VectorStock, Mailbox<VectorStock>, Address<VectorStock>),
    TruckStock(TruckStock, Mailbox<TruckStock>, Address<TruckStock>),
    LoadingProcess(LoadingProcess, Mailbox<LoadingProcess>, Address<LoadingProcess>),
    DumpingProcess(DumpingProcess, Mailbox<DumpingProcess>, Address<DumpingProcess>),
    TruckMovementProcess(TruckMovementProcess, Mailbox<TruckMovementProcess>, Address<TruckMovementProcess>),
}

impl ComponentModel {
    pub fn get_name(&self) -> &String {
        match self {
            ComponentModel::VectorStock(x, _, _) => &x.element_name,
            ComponentModel::TruckStock(x, _, _) => &x.element_name,
            ComponentModel::LoadingProcess(x, _, _) => &x.element_name,
            ComponentModel::DumpingProcess(x, _, _) => &x.element_name,
            ComponentModel::TruckMovementProcess(x, _, _) => &x.element_name,
        }
    }
}

#[derive(Debug, Clone)]
pub struct TruckAndOre {
    pub truck: i32,
    pub ore: VectorResource,
}