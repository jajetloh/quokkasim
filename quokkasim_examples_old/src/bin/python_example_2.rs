use nexosim::model::Model;
use nexosim::ports::{EventSource, EventQueue, Output};
use nexosim::registry::EndpointRegistry;
use nexosim::simulation::{Mailbox, SimInit, Simulation, SimulationError};
use nexosim::time::MonotonicTime;
use nexosim::server;

#[derive(Default)]
pub(crate) struct AddOne {
    pub(crate) output: Output<u16>
}

impl AddOne {
    pub async fn input(&mut self, value: u16) {
        self.output.send(value + 1).await;
    }
}

impl Model for AddOne {}

fn bench(_cfg: ()) -> Result<(Simulation, EndpointRegistry), SimulationError> {
    let mut model = AddOne::default();

    let model_mbox = Mailbox::new();
    let model_addr = model_mbox.address();

    let mut registry = EndpointRegistry::new();

    let output = EventQueue::new();
    model.output.connect_sink(&output);
    registry.add_event_sink(output.into_reader(), "add_1_output").unwrap();

    let mut input = EventSource::new();
    input.connect(AddOne::input, &model_addr);
    registry.add_event_source(input, "add_1_input").unwrap();

    let sim = SimInit::new()
        .add_model(model, model_mbox, "Adder")
        .init(MonotonicTime::EPOCH)?
        .0;

    Ok((sim, registry))
}


fn main() {
    server::run(bench, "127.0.0.1:12345".parse().unwrap()).unwrap();
}