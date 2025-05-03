use std::error::Error;
use std::time::Duration;

use crate::components::process::{DumpingProcess, LoadingProcess, TruckMovementProcess};
use crate::components::stock::TruckStock;
use crate::components::{ComponentModel, TruckAndOre};
use crate::loggers::{create_logger, EventLogger, Logger};
use crate::model_construction::{connect_components, ComponentModelAddress, ModelConfig};
use crate::ParsedArgs;
use indexmap::IndexMap;
use nexosim::time::MonotonicTime;
use quokkasim::core::{NotificationMetadata, Process, ResourceAdd, SimInit, Stock};
use quokkasim::prelude::{VectorResource, VectorStock};
use quokkasim::core::DistributionFactory;

pub fn build_and_run_model(args: ParsedArgs, config: ModelConfig) {

    let base_seed = args.seed;

    let mut df: DistributionFactory = DistributionFactory {
        base_seed,
        next_seed: base_seed,
    };

    let mut loggers: IndexMap<String, EventLogger> = IndexMap::new();
    for config in config.loggers {

        let logger_result: Result<EventLogger, Box<dyn Error>> = create_logger(config);
        match logger_result {
            Ok(logger) => {
                loggers.insert(logger.get_name().clone(), logger);
            }
            Err(e) => {
                eprintln!("Error creating logger: {}", e);
            }
        }
    }

    // let component_configs: Vec<ComponentConfig> = vec![];
    let mut components: IndexMap<String, ComponentModel> = IndexMap::new();
    for config in config.components {
        match config.create_component(&mut df, &mut loggers) {
            Ok(component) => {
                components.insert(component.get_name().clone(), component);
            }
            Err(e) => {
                eprintln!("Error creating component: {}", e);
            }
        }
    }

    // let connections_configs: Vec<ConnectionConfig> = vec![];
    let mut connection_errors: Vec<String> = vec![];

    for connection in config.connections {
        let comp_us = components.swap_remove(&connection.upstream);
        let comp_ds = components.swap_remove(&connection.downstream);
        match (comp_us, comp_ds) {
            (Some(comp1), Some(comp2)) => {
                // println!("Connecting {} to {}", comp1.get_name(), comp2.get_name());
                match connect_components(comp1, comp2) {
                    Ok((comp1, comp2)) => {
                        components.insert(connection.upstream.clone(), comp1);
                        components.insert(connection.downstream.clone(), comp2);
                    }
                    Err(e) => {
                        connection_errors.push(e.to_string());
                    }
                }
            }
            (Some(_), None) => {
                connection_errors.push(format!("Connection error: Component instance {} not defined", connection.downstream));
            },
            (None, Some(_)) => {
                connection_errors.push(format!("Connection error: Component instance {} not defined", connection.upstream));
            },
            (None, None) => {
                connection_errors.push(format!("Connection error: Component instances {} and {} not defined", connection.upstream, connection.downstream));
            }
        }
    }

    if !connection_errors.is_empty() {
        for error in connection_errors {
            eprintln!("{}", error);
            println!("{}", error);
        }
    }

    match components.get_mut(&config.truck_init_location) {
        Some(ComponentModel::TruckStock(comp, _, _)) => {
            (0..args.num_trucks).for_each(|i| {
                comp.resource.add(Some(TruckAndOre {
                    truck: (100 + i) as i32,
                    ore: VectorResource { vec: [0.; 5] },
                }));
            });
        },
        _ => {
            eprintln!("Truck init component not found");
            return;
        }
    };

    let mut sim_init = SimInit::new();
    let mut addresses: IndexMap<String, ComponentModelAddress> = IndexMap::new();

    for (_, component) in components.drain(..) {
        match component {
            ComponentModel::VectorStock(stock, mbox, addr) => {
                let element_name = stock.element_name.clone();
                addresses.insert(stock.element_name.clone(), ComponentModelAddress::ArrayStock(addr));
                sim_init = sim_init.add_model(stock, mbox, element_name);
            },
            ComponentModel::TruckStock(stock, mbox, addr) => {
                let element_name = stock.element_name.clone();
                addresses.insert(stock.element_name.clone(), ComponentModelAddress::TruckStock(addr));
                sim_init = sim_init.add_model(stock, mbox, element_name);
            },
            ComponentModel::LoadingProcess(loading, mbox, addr) => {
                let element_name = loading.element_name.clone();
                addresses.insert(loading.element_name.clone(), ComponentModelAddress::LoadingProcess(addr));
                sim_init = sim_init.add_model(loading, mbox, element_name);
            },
            ComponentModel::DumpingProcess(dumping, mbox, addr) => {
                let element_name = dumping.element_name.clone();
                addresses.insert(dumping.element_name.clone(), ComponentModelAddress::DumpingProcess(addr));
                sim_init = sim_init.add_model(dumping, mbox, element_name);
            },
            ComponentModel::TruckMovementProcess(movement, mbox, addr) => {
                let element_name = movement.element_name.clone();
                addresses.insert(movement.element_name.clone(), ComponentModelAddress::TruckMovementProcess(addr));
                sim_init = sim_init.add_model(movement, mbox, element_name);
            },
        }
    }

    let start_time = MonotonicTime::try_from_date_time(2025, 1, 1, 0, 0, 0, 0).unwrap();
    let mut simu = sim_init.init(start_time).unwrap().0;

    addresses.iter().for_each(|(name, addr)| {
        let nm = NotificationMetadata {
            time: start_time,
            element_from: name.clone(),
            message: "Start".into(),
        };
        match addr {
            ComponentModelAddress::ArrayStock(addr) =>  {
                simu.process_event(VectorStock::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::TruckStock(addr) => {
                simu.process_event(TruckStock::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::LoadingProcess(addr) => {
                simu.process_event(LoadingProcess::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::DumpingProcess(addr) => {
                simu.process_event(DumpingProcess::check_update_state, nm, addr).unwrap();
            },
            ComponentModelAddress::TruckMovementProcess(addr) => {
                simu.process_event(TruckMovementProcess::check_update_state, nm, addr).unwrap();
            },
        }
    });

    simu.step_until(start_time + Duration::from_secs_f64(args.sim_duration_secs)).unwrap();

    // Create dir if doesn't exist
    let dir = format!("outputs/trucking/{:04}", base_seed);
    if !std::path::Path::new(&dir).exists() {
        std::fs::create_dir_all(&dir).unwrap();
    }

    // loggers.iter_mut
    
    for logger in loggers.drain(..) {
        match logger {
            (_, EventLogger::TruckingProcessLogger(logger)) => {
                let logger_name = logger.get_name().clone();
                logger.write_csv(dir.clone()).unwrap_or_else(|e| {
                    eprintln!("Error writing logger {} to CSV: {}", logger_name, e);
                });
            },
            (_, EventLogger::QueueStockLogger(logger)) => {
                let logger_name = logger.get_name().clone();
                logger.write_csv(dir.clone()).unwrap_or_else(|e| {
                    eprintln!("Error writing logger {} to CSV: {}", logger_name, e);
                });
            },
            (_, EventLogger::ArrayStockLogger(logger)) => {
                let logger_name = logger.get_name().clone();
                logger.write_csv(dir.clone()).unwrap_or_else(|e| {
                    eprintln!("Error writing logger {} to CSV: {}", logger_name, e);
                });
            },
        }
    }
}