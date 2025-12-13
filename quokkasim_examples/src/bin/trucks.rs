// Goal: Have a component to represent trucks or similar?

/*
 * Processing area is parameterised by:
 * - Number of trucks
 * - Cycle time per route
 * - Route allocation
 * - Payload?
 */

use quokkasim::{components::mixed::{DefaultLoadingProcess, DefaultUnloadingProcess, LoadResource}, prelude::{ContResource, ContStockLog, ContStockState, DefaultContStock, DefaultDiscSink, DefaultDiscStock, DiscProcessLog, DiscStockLog, DiscStockState, Projectable}};
use serde::Serialize;

#[derive(Clone, Debug, Serialize, Default)]
struct Truck<ResourceType> where ResourceType: ContResource + Projectable<f64> {
    element_name: String,
    element_type: String,
    element_code: String,
    capacity: f64,
    contents: Option<ResourceType>
}

impl<ResourceType> LoadResource<ResourceType> for Truck<ResourceType> where ResourceType: ContResource + Projectable<f64>  {
    fn load_resource(&mut self, resource: ResourceType) {
        self.contents = Some(resource);
    }

    fn unload_resource(&mut self, quantity: f64) -> ResourceType {
        match self.contents.as_mut() {
            Some(res) => {
                if res.total() <= quantity {
                    return self.unload_resource_all();
                } else {
                    let moved = res.remove(quantity);
                    return moved;
                }
            },
            None => {
                ResourceType::default()
            }
        }
    }

    fn unload_resource_all(&mut self) -> ResourceType {
        self.contents.take().unwrap()
    }
}

fn main() {

    let mut stockpile_1: DefaultContStock<f64, ContStockState, ContStockLog<f64>> = DefaultContStock::new()
        .with_name("Stockpile1")
        .with_low_capacity(1.)
        .with_max_capacity(500.);
    let mut empty_trucks: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("EmptyTrucks")
        .with_low_capacity(0)
        .with_max_capacity(10);
    let mut loading_area: DefaultLoadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultLoadingProcess::new()
        .with_name("LoadingArea")
        .with_process_time_distr(
            quokkasim::distributions::DistributionFactory::new(12345)
                .create(quokkasim::distributions::DistributionConfig::Constant(30.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            quokkasim::distributions::DistributionFactory::new(12345)
                .create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let mut loading_complete_trucks: DefaultDiscStock<Truck<f64>, DiscStockState, DiscStockLog<Truck<f64>>> = DefaultDiscStock::new()
        .with_name("LoadingCompleteTrucks")
        .with_low_capacity(0)
        .with_max_capacity(10);

    let mut unloading_area: DefaultUnloadingProcess<Truck<f64>, DiscProcessLog<Truck<f64>>, f64> = DefaultUnloadingProcess::new()
        .with_name("UnloadingArea")
        .with_process_time_distr(
            quokkasim::distributions::DistributionFactory::new(12345)
                .create(quokkasim::distributions::DistributionConfig::Constant(30.0))
                .unwrap(),
        )
        .with_process_quantity_distr(
            quokkasim::distributions::DistributionFactory::new(12345)
                .create(quokkasim::distributions::DistributionConfig::Constant(1.0))
                .unwrap(),
        );
    let (unloading_mbox, unloading_addr) = unloading_area.create_mailbox();

    

}