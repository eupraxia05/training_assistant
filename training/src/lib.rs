//! A core plugin for training administration.

use framework::prelude::*;
use framework_derive_macros::TableRow;

#[derive(Clone)]
pub struct TrainingPlugin;

impl Plugin for TrainingPlugin {
    fn build(self, context: &mut Context) {
        context.add_table::<Trainer>("trainer")
            .add_table::<Client>("client");
    }
}

/// Stores information about a trainer. Useful for holding company details.
#[derive(TableRow, Debug)]
pub struct Trainer {
    name: String,
    company_name: String,
    address: String,
    email: String,
    phone: String,
}

impl Trainer {
    /// Gets the trainer's name.
    pub fn name(&self) -> &String {
        &self.name
    }

    /// Gets the trainer's company name.
    pub fn company_name(&self) -> &String {
        &self.company_name
    }

    /// Gets the trainer's address.
    pub fn address(&self) -> &String {
        &self.address
    }

    /// Gets the trainer's email address.
    pub fn email(&self) -> &String {
        &self.email
    }

    /// Gets the trainer's phone number.
    pub fn phone(&self) -> &String {
        &self.phone
    }
}

/// Contains data about a single training client.
#[derive(TableRow, Debug)]
pub struct Client {
    // The client's name.
    name: String,
}

impl Client {
    /// Gets a client's name.
    pub fn name(&self) -> &String {
        &self.name
    }
}
