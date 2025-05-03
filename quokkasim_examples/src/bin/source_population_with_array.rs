use std::time::Duration;

use nexosim::{simulation::Address, time::MonotonicTime};
use quokkasim::{
    common::Logger,
    components::vector::{VectorProcessLogger, VectorResource, VectorSource, VectorStock, VectorStockLogger},
    core::{
        DistributionConfig, DistributionFactory, Mailbox, NotificationMetadata, SimInit, Source, Stock
    },
};
use unzip3::Unzip3;

/**
 * This example demonstrates how an arbitrary number of processes can access a stock. This pattern
 * works as long as the stock does not need to differentiate between processes.
 * 
 * Example use cases:
 * - A number of processes all feeding from the same stock
 * - A number of processes all pushing to the same stock
 * 
 * If the stock needs to differentiate between processes, this pattern is not suitable. For example:
 * - A stock requesting the state of a single active instance of the process, within the population.
 */
fn main() {

    const NUM_SOURCES: usize = 50;

    let process_logger: VectorProcessLogger = VectorProcessLogger::new("ProcessLogger".into(), 100_000);
    let stock_logger: VectorStockLogger = VectorStockLogger::new("StockLogger".into(), 100_000);
    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    // First, population of processes...

    let mut stock = VectorStock::new()
        .with_name("Stock1".into())
        .with_log_consumer(&stock_logger);
    let stock_mbox: Mailbox<VectorStock> = Mailbox::new();
    let stock_addr = stock_mbox.address();
    stock.low_capacity = 0.;
    stock.max_capacity = 10_000.;

    let (mut sources, source_mboxes, source_addrs): (Vec<VectorSource>, Vec<Mailbox<VectorSource>>, Vec<Address::<VectorSource>>) = (0..NUM_SOURCES).into_iter().map(|i| {
        let mut source = VectorSource::new()
            .with_name(format!("Source{}", i).into())
            .with_time_to_new_dist(df.create(DistributionConfig::Triangular { min: 10., mode: 50., max: 60. }).unwrap());
        source.log_emitter.connect_sink(process_logger.get_buffer());
        let source_mbox: Mailbox<VectorSource> = Mailbox::new();
        let source_addr: Address<VectorSource> = source_mbox.address();
        source.component_split = Some(VectorResource { vec: [0.4, 0.3, 0.2, 0.1, 0.0 ]});
        source.create_quantity_dist = Some(df.create(DistributionConfig::TruncNormal { mean: 1., std: 0.5, min: Some(0.), max: None }).unwrap());
        (source, source_mbox, source_addr)
    }).collect::<Vec<(VectorSource, Mailbox<VectorSource>, Address<VectorSource>)>>().into_iter().unzip3();

    // Connections

    sources.iter_mut().zip(&source_addrs).for_each(|(source, source_addr)| {
        source.req_downstream.connect(VectorStock::get_state, &stock_addr);
        source.push_downstream.connect(VectorStock::add, &stock_addr);
        stock.state_emitter.connect(VectorSource::check_update_state, source_addr);
    });

    // Run

    let mut sim_builder = SimInit::new()
        .add_model(stock, stock_mbox, "Stock1");
    for (source, source_mbox) in sources.into_iter().zip(source_mboxes.into_iter()) {
        let element_name = source.element_name.clone();
        sim_builder = sim_builder.add_model(source, source_mbox, element_name);
    }
    
    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_builder.init(start_time).unwrap().0;
    for source_addr in source_addrs.iter() {
        simu.process_event(
            VectorSource::check_update_state,
            NotificationMetadata {
                    time: start_time,
                    element_from: "Simulation".into(),
                    message: "Start".into(),
                },
            source_addr
        ).unwrap();
    };

    simu.step_until(start_time + Duration::from_secs(60 * 60 * 2)).unwrap();

    std::fs::create_dir_all("outputs/source_population_with_vector").unwrap();
    process_logger.write_csv("outputs/source_population_with_vector".into()).unwrap();
    stock_logger.write_csv("outputs/source_population_with_vector".into()).unwrap();
}