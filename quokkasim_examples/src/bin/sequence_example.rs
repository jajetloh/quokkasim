use quokkasim::prelude::*;
use quokkasim::define_model_enums;
use std::error::Error;

define_model_enums! {
    pub enum ComponentModel<'a> {}
    pub enum ComponentLogger<'a> {}
}


impl<'a> CustomComponentConnection for ComponentModel<'a> {
    fn connect_components(a: Self, b: Self) -> Result<(), Box<dyn Error>> {
        Err(format!("connect_components not implemented from {} to {}", a, b).into())
    }
}

impl<'a> CustomLoggerConnection<'a> for ComponentLogger<'a> {
    type ComponentType = ComponentModel<'a>;
    fn connect_logger(a: Self, b: Self::ComponentType) -> Result<(), Box<dyn Error>> {
        Err(format!("connect_logger not implemented from {} to {}", a, b).into())
    }
}

fn main() {
    let mut stock1: SequenceStock<String> = SequenceStock::<String>::new()
        .with_name("Stock1".into())
        .with_type("SequenceStockU32".into())
        .with_initial_contents((0..10).map(|x| format!("Item_{:0>4}", x)).collect());
    let stock1_mbox: Mailbox<SequenceStock<String>> = Mailbox::new();
    let mut stock1_addr = stock1_mbox.address();

    let mut process1: SequenceProcess<Option<String>, (), Option<String>> = SequenceProcess::new()
        .with_name("Process1".into())
        .with_type("SequenceProcess".into());
    let process1_mbox: Mailbox<SequenceProcess<Option<String>, (), Option<String>>> = Mailbox::new();
    let mut process1_addr = process1_mbox.address();

    let mut stock2: SequenceStock<String> = SequenceStock::<String>::new()
        .with_name("Stock2".into())
        .with_type("SequenceStockU32".into());
    let stock2_mbox: Mailbox<SequenceStock<String>> = Mailbox::new();
    let mut stock2_addr = stock2_mbox.address();

    stock1.state_emitter.connect(
        <
            SequenceProcess< Option<String>, (), Option<String> >
            as Process< SeqDeque<String>, Option<String>, (), u32 >
        >::update_state, &process1_addr);
}