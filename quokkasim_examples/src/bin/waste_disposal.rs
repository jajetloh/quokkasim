use quokkasim::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;

fn create_bench() -> SimInit {
    let mut df = DistributionFactory::new(55555);

    let mut dump_source: DefaultSource<f64, ContinuousProcessLog<DefaultProcessLogType<f64>, f64>> =
        DefaultSource::new()
            .with_name("DumpSource")
            .with_code("DS")
            .with_source_resource(1.0)
            .with_source_quantity_distr(
                df.create(DistributionConfig::Constant(50.0))
                    .unwrap(),
            );
    let ds_mbox = Mailbox::new();
    let ds_addr = ds_mbox.address();

    let mut dump_point: DefaultStock<f64, ContinuousStockState, ContinuousStockLog<f64>> = DefaultStock::new()
        .with_name("DumpPoint")
        .with_code("DP")
        .with_low_capacity(1.)
        .with_max_capacity(10_000.)
        .with_initial_resource(0.);
    let dp_mbox = Mailbox::new();
    let dp_addr = dp_mbox.address();

    let mut material_sink: DefaultSink<f64, ContinuousProcessLog<DefaultProcessLogType<f64>, f64>> =
        DefaultSink::new()
            .with_name("MaterialSink")
            .with_code("MS")
            .with_sink_quantity_distr(
                df.create(DistributionConfig::Constant(50.0))
                    .unwrap(),
            )
            .with_sink_time_distr(df.create(DistributionConfig::Constant(1.0)).unwrap());
    let ms_mbox = Mailbox::new();
    let ms_addr = ms_mbox.address();

    let sim_init = SimInit::new()
        .add_model(dump_source, ds_mbox, "DumpSource")
        .add_model(dump_point, dp_mbox, "DumpPoint")
        .add_model(material_sink, ms_mbox, "MaterialSink");
    sim_init
}

fn main() {
    create_bench();
}