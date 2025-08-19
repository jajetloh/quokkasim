

define_model_enums! {
    pub enum ComponentModel {}
    pub enum ComponentModelAddress {}
    pub enum ComponentLogger {}
    pub enum ScheduledEvent {}
}

impl CustomComponentConnection for ComponentModel {
    fn connect_components(
        a: &mut Self,
        b: &mut Self,
        n: Option<usize>
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (a, b) {
            (a, b) => Err(format!(
                "No component connection defined from {} to {} (n={:?})",
                a, b, n
            )
            .into()),
        }
    }
}


impl CustomLoggerConnection for ComponentLogger {
    type ComponentType = ComponentModel;

    fn connect_logger(
        a: &mut Self,
        b: &mut Self::ComponentType,
        n: Option<usize>
    ) -> Result<(), Box<dyn std::error::Error>> {
        match (a, b, n) {
            (a, b, _) => Err(format!(
                "No logger connection defined from {} to {} (n={:?})",
                a, b, n
            )
            .into()),
        }
    }
}
fn main() {
    println!("Hello, QuokkaSim!");
}
