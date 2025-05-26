use std::collections::VecDeque;
use std::{error::Error, fs::create_dir_all, time::Duration};
use quokkasim::nexosim::Mailbox;
use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use serde::{ser::SerializeStruct, Serialize};
use loggers::*;


#[derive(Debug, Clone)]
struct IronOre {
    fe: f64,
    other_elements: f64,
    magnetite: f64,
    hematite: f64,
    limonite: f64,
}

impl Default for IronOre {
    fn default() -> Self {
        IronOre {
            fe: 0.0,
            other_elements: 0.0,
            magnetite: 0.0,
            hematite: 0.0,
            limonite: 0.0,
        }
    }
}

impl VectorArithmetic<IronOre, f64, f64> for IronOre {
    fn add(&mut self, other: Self) {
        self.fe += other.fe;
        self.other_elements += other.other_elements;
        self.magnetite += other.magnetite;
        self.hematite += other.hematite;
        self.limonite += other.limonite;
    }

    fn subtract_parts(&self, quantity: f64) -> SubtractParts<IronOre, IronOre> {
        let proportion_removed = quantity / self.total();
        let proportion_remaining = 1.0 - proportion_removed;
        SubtractParts {
            remaining: IronOre {
                fe: self.fe * proportion_remaining,
                other_elements: self.other_elements * proportion_remaining,
                magnetite: self.magnetite * proportion_remaining,
                hematite: self.hematite * proportion_remaining,
                limonite: self.limonite * proportion_remaining,
            },
            subtracted: IronOre {
                fe: self.fe * proportion_removed,
                other_elements: self.other_elements * proportion_removed,
                magnetite: self.magnetite * proportion_removed,
                hematite: self.hematite * proportion_removed,
                limonite: self.limonite * proportion_removed,
            },
        }
    }

    // We use the Fe + Other Elements as the 'source of truth' for the total mass
    fn total(&self) -> f64 {
        self.fe + self.other_elements
    }
}

#[derive(Clone, Debug)]
struct TruckAndIronOre { 
    ore: Option<IronOre>, 
    truckid: String,
}

impl Default for TruckAndIronOre {
    fn default() -> Self {
        TruckAndIronOre {
            ore: Default::default(),
            truckid: "1".into()
        }
    }
}

#[derive(Clone, Debug)]
struct SeqDeque {
    deque: VecDeque<TruckAndIronOre>
}

impl Default for SeqDeque {
    fn default() -> Self {
        SeqDeque {
            deque: VecDeque::<TruckAndIronOre>::new()
        }
    }
}

impl VectorArithmetic<Option<TruckAndIronOre>, (), u32> for SeqDeque where TruckAndIronOre: Clone {
    fn add(&mut self, other: Option<TruckAndIronOre>) {
        match other {
            Some(item) => {
                self.deque.push_back(item);
            }
            None => {}
        }
    }

    fn subtract_parts(&self, _: ()) -> SubtractParts<Self, Option<TruckAndIronOre>> {
        let mut cloned = self.clone();
        let subtracted = cloned.deque.pop_front();
        SubtractParts {
            remaining: cloned,
            subtracted: subtracted
        }
    }

    fn total(&self) -> u32 {
        self.deque.len() as u32
    }
}

#[derive(Clone, Debug)]
struct TruckStock {
    pub element_name: String,
    pub element_type: String,
    pub sequence: SeqDeque,
    pub log_emitter: Output<TruckStockLog>,
    pub state_emitter: Output<NotificationMetadata>,
    pub low_capacity: u32,
    pub max_capacity: u32,
    pub prev_state: Option<TruckStockState>,
    next_event_id: u64,
}

impl Default for TruckStock {
    fn default() -> Self {
        TruckStock {
            element_name: "TruckStock".to_string(),
            element_type: "TruckStock".to_string(),
            sequence: SeqDeque::default(),
            log_emitter: Output::new(),
            state_emitter: Output::new(),
            low_capacity: 0,
            max_capacity: 1,
            prev_state: None,
            next_event_id: 0,
        }
    }
}

impl Stock<SeqDeque, Option<TruckAndIronOre>, (), Option<TruckAndIronOre>, u32> for TruckStock where Self: Model {
    type StockState = TruckStockState;
    fn get_state(&mut self) -> Self::StockState {
        let occupied = self.sequence.total();
        let empty = self.max_capacity.saturating_sub(occupied); // If occupied beyond capacity, just say no empty space
        if self.sequence.total() <= self.low_capacity {
            TruckStockState::Empty { occupied, empty }
        } else if self.sequence.total() >= self.max_capacity {
            TruckStockState::Full { occupied, empty }
        } else {
            TruckStockState::Normal { occupied, empty }
        }
    }
    fn get_previous_state(&mut self) -> &Option<Self::StockState> {
        &self.prev_state
    }
    fn set_previous_state(&mut self) {
        self.prev_state = Some(self.get_state());
    }
    fn get_resource(&self) -> &SeqDeque {
        &self.sequence
    }
    fn add_impl<'a>(&'a mut self, payload: &'a (Option<TruckAndIronOre>, NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = ()> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            match payload.0 {
                Some(ref item) => {
                    self.sequence.deque.push_back(item.clone());
                }
                None => {}
            }
        }
    }
    fn remove_impl<'a>(&'a mut self, payload: &'a ((), NotificationMetadata), cx: &'a mut Context<Self>) -> impl Future<Output = Option<T>> + 'a {
        async move {
            self.prev_state = Some(self.get_state());
            self.sequence.deque.pop_front()
        }
    }

    fn emit_change<'a>(&'a mut self, payload: NotificationMetadata, cx: &'a mut nexosim::model::Context<Self>) -> impl Future<Output = ()> + Send + 'a {
        async move {
            self.state_emitter.send(payload).await;
            self.log(cx.time(), "Emit Change".to_string()).await;
        }
    }

    fn log(&mut self, time: MonotonicTime, log_type: String) -> impl Future<Output = ()> + Send {
        async move {
            let log = TruckStockLog {
                time: time.to_chrono_date_time(0).unwrap().to_string(),
                event_id: self.next_event_id,
                element_name: self.element_name.clone(),
                element_type: self.element_type.clone(),
                log_type,
                state: self.get_state(),
                sequence: self.sequence.clone(),
            };
            self.log_emitter.send(log).await;
            self.next_event_id += 1;
        }
    }
}

impl Model for TruckStock {}

impl TruckStock {
    pub fn new() -> Self {
        TruckStock::default()
    }
    pub fn with_name(mut self, name: String) -> Self {
        self.element_name = name;
        self
    }
    pub fn with_type(mut self, type_: String) -> Self {
        self.element_type = type_;
        self
    }

    pub fn with_initial_contents(mut self, contents: Vec<TruckAndIronOre>) -> Self {
        self.sequence.deque = contents.into_iter().collect();
        self
    }

    pub fn with_low_capacity(mut self, low_capacity: u32) -> Self {
        self.low_capacity = low_capacity;
        self
    }

    pub fn with_max_capacity(mut self, max_capacity: u32) -> Self {
        self.max_capacity = max_capacity;
        self
    }
}






define_model_enums! {
    pub enum ComponentModel<'a> {
        // IronOreProcess(&'a mut VectorProcess<IronOre, IronOre, f64>, &'a mut Address<VectorProcess<IronOre, IronOre, f64>>),
        // IronOreStock(&'a mut VectorStock<IronOre>, &'a mut Address<VectorStock<IronOre>>),
        IronOreStock(VectorStock<IronOre>, Mailbox<VectorStock<IronOre>>),
        TruckStock(TruckStock, Mailbox<TruckStock>),
    }
    pub enum ComponentLogger<'a> {
        IronOreStockLogger(loggers::IronOreStockLogger),
        //TODO: build truck and iron ore logger
    }
}

impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            // (ComponentModel::IronOreProcess(a, ad), ComponentModel::IronOreStock(b, bd)) => {
            //     b.state_emitter.connect(VectorProcess::<IronOre, IronOre, f64>::update_state, ad.clone());
            //     a.req_downstream.connect(VectorStock::<IronOre>::get_state_async, bd.clone());
            //     a.push_downstream.connect(VectorStock::<IronOre>::add, bd.clone());
            //     Ok(())
            // },
            // (ComponentModel::IronOreStock(a, ad), ComponentModel::IronOreProcess(b, bd)) => {
            //     a.state_emitter.connect(VectorProcess::<IronOre, IronOre, f64>::update_state, bd.clone());
            //     b.req_upstream.connect(VectorStock::<IronOre>::get_state_async, ad.clone());
            //     b.withdraw_upstream.connect(VectorStock::<IronOre>::remove, ad.clone());
            //     Ok(())
            // },
            _ => Err("Invalid connection".into()),
        }
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        match (a, b) {
            // (ComponentLogger::IronOreProcessLogger(a), ComponentModel::IronOreProcess(b, _)) => {
            //     b.log_emitter.map_connect_sink(|c| <VectorProcessLog<IronOre>>::into(c.clone()), &a.buffer);
            //     Ok(())
            // },
            // (ComponentLogger::IronOreStockLogger(a), ComponentModel::IronOreStock(b, _)) => {
            //     b.log_emitter.map_connect_sink(|c| <VectorStockLog<IronOre>>::into(c.clone()), &a.buffer);
            //     Ok(())
            // },
            _ => Err("Invalid connection".into()),
        }
    }
}



fn main() {

    let base_seed = 987654321;
    let mut df = DistributionFactory::new(base_seed);

    let mut source_sp = ComponentModel::IronOreStock(
        VectorStock::new()
            .with_name("source_sp".into())
            .with_type("IronOreStock".into())
            .with_initial_vector(IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. })
            .with_low_capacity(10.)
            .with_max_capacity(100.),
        Mailbox::new(),
    );

    let mut stk_ready_to_load = ComponentModel::TruckStock(
        TruckStock::default()
            .with_name("stk_ready_to_load".into())
            .with_type("TruckStock".into())
            .with_initial_contents(vec![TruckAndIronOre { ore: Some(IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. }), truckid: "1".into() }])
            .with_low_capacity(0)
            .with_max_capacity(3),
        Mailbox::new(),
    );

    let mut stk_loaded = ComponentModel::TruckStock(
        TruckStock::default()
            .with_name("stk_loaded".into())
            .with_type("TruckStock".into())
            .with_initial_contents(vec![TruckAndIronOre { ore: Some(IronOre { fe: 60., other_elements: 40., magnetite: 10., hematite: 5., limonite: 15. }), truckid: "2".into() }])
            .with_low_capacity(0)
            .with_max_capacity(3),
        Mailbox::new(),
    );

    

    


    


    // following sequence example
    // TODO: create 2 stocks of TruckAndIronOre
    // TODO: create process to transfer resource from one stock to the other

    // truck + iron ore stock





    println!("hello, world!")

}
