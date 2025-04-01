# QuokkaSim

A Rust-based Discrete Event Simulation (DES) framework, which simplifies simulation for beginners, and provides excellent utilities for experience modellers.

Powered by the [NeXosim](https://github.com/asynchronics/nexosim) simulation engine for Rust.

---

Many frameworks, commercial applications and code libraries exist for performing simulation. QuokkaSim aims to be a free, open-source alternative, which leverages the performance, safety and features of Rust to minimise time spent by modellers on non-value-add tasks, such as fixing uninitialised components, and writing component boilerplate.

QuokkaSim does this by using a very flexible Stock-and-Flow approach.

**Components** are plug-and-play objects which can be easily configured and used straightaway in your model, alongside other components.

However, if our pre-defined components aren't suitable for your use case, you can easily create your own custom components that fit within the QuokkaSim framework using **Definition Macros**.

# Component Guide

In QuokkaSim, we use Resources, Stocks and Processes to represent quantities of material and  interacting entities.

## Resources

**Resources** are quantities/tangible things that can move and change over the simulation, and that we are interested in tracking.

e.g. If we are simulating the operation of a Cafe with its staff and drinks being ordered:
- **Customers** may be one type of resource as they come in and out of the Cafe.
- **Drinks** may be another type of resource, as we are interested in how they move through the stages of being made before being handed to the customer.

e.g. If we are simulating a manufacturing plant:
- Different **parts** may be one type of resource, along with relevant physical information like size, defects, compliance to spec.
- **Electricity** may be another resource we want to track, in order to account for the energy cost of running certain equipment.

## Stocks

**Stocks** are constructs that hold resources.

e.g. Using the Cafe example:
- The **queue** of customers is a stock that contains all customer who are waiting to be served.
- **Dry inventory** and the **fridge** may be stocks that hold quantities of each type of coffee bean, and types of milk respectively.

e.g. Using the manufacturing plant example:
- **Anywhere that parts can be placed** whilst waiting for the next step in the manufacturing process is a stock.
- We may choose to create a single stock that drains any time electricity is used, in order to have a tangible representation of energy consumption.

## Processes, Sources and Sinks

**Processes** are constructs that interact with stocks, and importantly handle the movements of resources to and from stocks.

e.g. Using the Cafe example:
- The **cashier** process interacts with the customer at the front of the queue, after which the customer moves to a "waiting for their order" stock. They may also create an **ticket** resource, which moves into the queue of tickets to be handled by the **barista** process.
- The **barista** process interacts with tickets and the stock of coffee beans and milks to create a **drink** resource, and interacts with the "waiting for their order" stock of customers to give the customer the completed drink.

e.g. Using the Manufacturing example:
- Any set of processes that must be completed in series between stocks, can be modelled as a single (or multiple) processes

In many cases, we must ensure that resources are conserved as part of our simulation. Even when there are losses (e.g. spillage of drinks, energy lost as heat etc.), it is good practice to have processes still conserve these resources, and instead model these losses explicitly with Sinks or Sources.

**Sources** are processes that create resources.

e.g. Using the Cafe example:
- **Customers entering** can be modelled as a source, which feeds into the queue stock.
e.g. Using the Manufacturing example:
- **Delivery or procurement** during the modelled time frame can be modelled as sources for their respective resource.

Similarly, **sinks** are processes that destroy resources.

e.g. Using the Cafe example:
- **Spillage** from the barista can be modelled as occasionally moving drinks into a "spilled" stock, which the Spillage sink removes from the simulation.
e.g. Using the Manufacturing example:
- Periodic disposal of a waste product from a waste stock, or dispatching of finished product to a warehouse, can both be modelled as sinks that remove resources from the simulation.

# Development Guide
- VSCode is recommended as an IDE due to its strong support for the ``rust-analyzer`` LSP, which provides Intellisense and error detection whilst writing your code.
- The ``quokkasim`` directory contains the main QuokkaSim crate code.
- The ``quokkasim_examples`` directory contains uses of QuokkaSim code, which can be run to test functionality and copied to quickly spin up new models.
    - To try an example model, use ``cargo run --bin [EXAMPLE_NAME e.g. discrete_queue]``
