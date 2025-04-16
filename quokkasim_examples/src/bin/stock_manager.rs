use nexosim::{model::Context, ports::Requestor, time::MonotonicTime};
// use quokkasim::{common::EventLogger, components::array::{ArrayProcessLog, ArrayResource, ArraySource, ArrayStockLog, ArrayStockState}, define_stock};
use quokkasim::prelude::*;

define_stock!(
    /// ffff
    name = TruckStock,
    resource_type = ArrayResource,
    initial_resource = ArrayResource { vec: [0.0; 5] },
    add_type = ArrayResource,
    remove_type = ArrayResource,
    remove_parameter_type = f64,
    state_type = ArrayStockState,
    fields = {
        capacity: f64,
        location: String
    },
    get_state_method = |stock: &TruckStock| -> ArrayStockState {
        let total = stock.resource.total();
        if total == 0. {
            ArrayStockState::Empty { occupied: total, remaining_capacity: stock.capacity - total }
        } else if total < stock.capacity {
            ArrayStockState::Normal { occupied: total, remaining_capacity: stock.capacity - total }
        } else {
            ArrayStockState::Full { occupied: total, remaining_capacity: 0. }
        }
    },
    check_update_method = |x: &mut Self, cx: &mut Context<Self>| {
    },
    log_record_type = ArrayStockLog,
    log_method = |x: &'a mut Self, time: MonotonicTime, log_type: String| {
        async move {
            let state = x.get_state().await;
            let log = ArrayStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                element_name: x.element_name.clone(),
                element_type: x.element_type.clone(),
                log_type,
                occupied: match state { 
                    ArrayStockState::Empty { occupied, .. } => occupied,
                    ArrayStockState::Normal { occupied, .. } => occupied,
                    ArrayStockState::Full { occupied, .. } => occupied,
                },
                remaining_capacity: match state {
                    ArrayStockState::Empty { remaining_capacity, .. } => remaining_capacity,
                    ArrayStockState::Normal { remaining_capacity, .. } => remaining_capacity,
                    ArrayStockState::Full { remaining_capacity, .. } => remaining_capacity,
                },
                state: match state {
                    ArrayStockState::Empty { .. } => "Empty".to_string(),
                    ArrayStockState::Normal { .. } => "Normal".to_string(),
                    ArrayStockState::Full { .. } => "Full".to_string(),
                },
                x0: x.resource.vec[0],
                x1: x.resource.vec[1],
                x2: x.resource.vec[2],
                x3: x.resource.vec[3],
                x4: x.resource.vec[4],
            };
            x.log_emitter.send(log).await;
        }
    }
);

struct TruckLoadingManager {
    req_downstream: Requestor<String /* Source location id */, ArrayStockState>,
    push_downstream: Requestor<String /* Source location id */, ArrayResource>,


}

fn main() {
    let process_logger: EventLogger<ArrayProcessLog> = EventLogger::new(100_000);

    let mut source = ArraySource::new()
        .with_name("Source".into())
        .with_time_to_new_dist(Distribution::Constant(60.))
        .with_log_consumer(&process_logger);
    let source_mbox: Mailbox<ArraySource> = Mailbox::new();
    let source_addr = source_mbox.address();
    source.component_split = Some(ArrayResource { vec: [0.8, 0.2, 0., 0., 0. ]});
    source.create_quantity_dist = Some(Distribution::Constant(1.));

    let mut stock_


}