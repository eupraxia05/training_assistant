//! A core plugin for training administration.

use framework::prelude::*;
use framework_derive_macros::TableRow;
use tui::prelude::*;
use chrono::NaiveDate;

/// The plugin for the Training system. 
/// Add this to set up the required tables and commands.
#[derive(Clone)]
pub struct TrainingPlugin;

impl Plugin for TrainingPlugin {
    fn build(self, context: &mut Context) -> Result<()> {
        context.add_table(TableConfig::new::<Trainer>("trainer"))
            .add_table(TableConfig::new::<Client>("client"))
            .add_table(TableConfig::new::<Exercise>("exercise"))
            .add_table(TableConfig::new::<Session>("session"));
       
        // TODO: conditionally compile this
        if let Some(new_tab_types) = context.get_resource_mut::<TuiNewTabTypes>() {
            new_tab_types.register_new_tab_type::<ScheduleTabImpl>("Schedule");
        }

        Ok(())
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

/// An exercise in the exercise library.
#[derive(TableRow, Debug)]
pub struct Exercise {
    name: String
}

// TODO: implement this
struct ScheduleTabImpl;

#[derive(Default)]
struct ScheduleTabState;

impl TabImpl for ScheduleTabImpl {
    type State = ScheduleTabState;

    fn title() -> String { "Schedule".into() }

    fn render(_: &mut Context, buffer: &mut Buffer, rect: Rect, block: Block, _: usize) {
        Paragraph::new(Line::from("Schedule UI not implemented.")).block(block).render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        vec!()
    }

    fn handle_key(_: &mut Context, _: &str, _: usize) {

    }
}

#[derive(TableRow, Debug)]
pub struct Session {
    date: NaiveDate,
    #[display_table("trainer", "name")]
    trainer: RowId,
    #[display_table("client", "name")]
    client: RowId,
    charge: Option<RowId>,
}

