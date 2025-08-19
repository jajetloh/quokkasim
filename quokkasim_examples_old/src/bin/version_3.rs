use quokkasim::v3::*;

struct MyNewStruct {}
impl<T: Resource> Process<T> for MyNewStruct {}

impl<T: Resource, S: StockState> Connect<MyNewStruct, DefaultStock<T, S>> for Connection {
    fn connect(&mut self, a: &mut MyNewStruct, b: &mut DefaultStock<T, S>) -> Result<(), String> {
        Ok(())
    }
}

struct MyWrapperProcess<T: Resource> {
    t: T
}
impl<T: Resource> Process<T> for MyWrapperProcess<T> {}

struct MyWrapperStock<T: Resource, S: StockState> {
    t: T,
    s: S
}
impl<T: Resource, S: StockState> Stock<T, S> for MyWrapperStock<T, S> {}

impl<T: Resource, S: StockState> Connect<MyWrapperProcess<T>, MyWrapperStock<T, S>> for Connection {
    fn connect(&mut self, a: &mut MyWrapperProcess<T>, b: &mut MyWrapperStock<T, S>) -> Result<(), String> {
        // Example connection logic
        Ok(())
    }
}

fn main() {
    let mut p = DefaultProcess { t: 1_f64 };
    let mut s = DefaultStock { t: 1_f64, s: ExampleStockState {} };
    let mut m = MyNewStruct {};

    let mut p2 = MyWrapperProcess { t: 1_f64 };
    let mut s2 = MyWrapperStock { t: 1_f64, s: ExampleStockState {} };

    Connection.connect(&mut m, &mut s).unwrap();
    Connection.connect(&mut p2, &mut s2).unwrap();
}