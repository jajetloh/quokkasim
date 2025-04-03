use nexosim::model::{Context, Model};
use nexosim::ports::{Output, Requestor};
use nexosim::simulation::{Mailbox, SimInit};
use nexosim::time::MonotonicTime;

struct MyStruct {
    output: Output<f64>,
}

impl Model for MyStruct {}

struct MyOtherStruct {
    requestor: Requestor<(), f64>
}

impl Model for MyOtherStruct {}

impl MyOtherStruct {
    async fn do_request(self) -> f64 {
        // let response = self.requestor.send(()).await.next().unwrap();
        // return response
        return 1.23
    }
}

fn main() {
    let my_struct = MyStruct {
        output: Output::new(),
    };
    let my_struct_mbox: Mailbox<MyStruct> = Mailbox::new();
    let my_struct_addr = my_struct_mbox.address();

    let my_other_struct = MyOtherStruct {
        requestor: Requestor::new(),
    };
    let my_other_struct_mbox: Mailbox<MyOtherStruct> = Mailbox::new();
    let my_other_struct_addr = my_other_struct_mbox.address();

    let simu = SimInit::new()
        // .add_model(my_struct, my_struct_mbox, "MyStruct")
        .add_model(my_other_struct, my_other_struct_mbox, "MyOtherStruct");

    let (mut sim, _) = simu.init(MonotonicTime::EPOCH).unwrap();

    sim.process_event(MyOtherStruct::do_request, (), my_other_struct_addr).unwrap();

}