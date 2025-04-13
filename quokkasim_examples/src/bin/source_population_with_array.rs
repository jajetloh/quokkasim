use std::time::Duration;

use nexosim::{simulation::Address, time::MonotonicTime};
use quokkasim::{
    common::EventLogger,
    components::array::{ArrayProcessLog, ArrayResource, ArraySource, ArrayStock, ArrayStockLog},
    core::{
        DistributionConfig, DistributionFactory, Mailbox, NotificationMetadata, SimInit, Source, Stock
    },
};
use unzip3::Unzip3;

fn main() {

    const NUM_SOURCES: usize = 50;

    let process_logger: EventLogger<ArrayProcessLog> = EventLogger::new(100_000);
    let stock_logger: EventLogger<ArrayStockLog> = EventLogger::new(100_000);
    let mut df = DistributionFactory {
        base_seed: 1234,
        next_seed: 0,
    };

    // First, population of processes...

    let mut stock = ArrayStock::new()
        .with_name("Stock1".into())
        .with_log_consumer(&stock_logger);
    let stock_mbox: Mailbox<ArrayStock> = Mailbox::new();
    let stock_addr = stock_mbox.address();
    stock.low_capacity = 0.;
    stock.max_capacity = 10_000.;

    let (mut sources, source_mboxes, source_addrs): (Vec<ArraySource>, Vec<Mailbox<ArraySource>>, Vec<Address::<ArraySource>>) = (0..NUM_SOURCES).into_iter().map(|i| {
        let mut source = ArraySource::new()
            .with_name(format!("Source{}", i).into())
            .with_time_to_new_dist(df.create(DistributionConfig::Triangular { min: 10., mode: 50., max: 60. }).unwrap())
            .with_log_consumer(&process_logger);
        let source_mbox: Mailbox<ArraySource> = Mailbox::new();
        let source_addr: Address<ArraySource> = source_mbox.address();
        source.component_split = Some(ArrayResource { vec: [0.4, 0.3, 0.2, 0.1, 0.0 ]});
        source.create_quantity_dist = Some(df.create(DistributionConfig::TruncNormal { mean: 1., std: 0.5, min: Some(0.), max: None }).unwrap());
        (source, source_mbox, source_addr)
    }).collect::<Vec<(ArraySource, Mailbox<ArraySource>, Address<ArraySource>)>>().into_iter().unzip3();

    // Connections

    sources.iter_mut().zip(&source_addrs).for_each(|(source, source_addr)| {
        source.req_downstream.connect(ArrayStock::get_state, &stock_addr);
        source.push_downstream.connect(ArrayStock::add, &stock_addr);
        stock.state_emitter.connect(ArraySource::check_update_state, source_addr);
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
            ArraySource::check_update_state,
            NotificationMetadata {
                    time: start_time,
                    element_from: "Simulation".into(),
                    message: "Start".into(),
                },
            source_addr
        ).unwrap();
    };

    simu.step_until(start_time + Duration::from_secs(60 * 60 * 2)).unwrap();

    process_logger.write_csv("outputs/source_population_with_array_process_logs.csv").unwrap();
    stock_logger.write_csv("outputs/source_population_with_array_stock_logs.csv").unwrap();
}