//! A plugin for generating invoices and tracking charges.
use clap::{Arg, ArgMatches, Command};
use documents::write_document;
use dolmen::prelude::*;
use framework::prelude::*;
use framework_derive_macros::TableRow;
use latex::{
    Document, DocumentClass, Element, PreambleElement,
};
use std::path::PathBuf;
use training::{Client, Trainer};

#[cfg(feature="tui")]
use tui::{TabImpl, KeyBind};

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph, Widget};
use ratatui::text::Line;
use gui::prelude::*;

///////////////////////////////////////////////////////////////////////////////
// PUBLIC API
///////////////////////////////////////////////////////////////////////////////
/// A `Plugin` that sets up the required commands and
/// tables for billing.
#[derive(Default, Clone)]
pub struct InvoicePlugin;

/// A table row storing a single issued invoice. Stored in the table
/// `invoice`.
#[derive(TableRow, Debug)]
pub struct Invoice {
    /// The row ID in the `client` table representing the client paying the
    /// invoice.
    pub client: RowId,

    /// The row ID in the `trainer` table representing the trainer issuing the
    /// invoice.
    pub trainer: RowId,

    /// An invoice number, in any desired format.
    pub invoice_number: String,

    /// The due date of the invoice.
    pub due_date: String,

    /// The date the invoice was paid.
    pub date_paid: String,

    /// The method by which the invoice was paid (cash, payment processor, etc)
    pub paid_via: String,

    /// The list of charges this invoice covers.
    pub charges: Vec<RowId>,
}

/// A table row storing a single issued charge. Stored in the table
/// `charge`.
#[derive(TableRow, Debug)]
pub struct Charge {
    /// The date the charge was issued.
    pub date: String,

    /// A description of the charge
    /// (e.g. `"Personal training session (60 min)"`)
    pub description: String,

    /// The amount charged, in dollars.
    // TODO: allow for other non-Murican currencies
    pub amount: i32,
}

///////////////////////////////////////////////////////////////////////////////
// PRIVATE IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////
impl Plugin for InvoicePlugin {
    fn build(self, context: &mut Context) -> dolmen::Result<()> {
        // set up charge and invoice tables
        context
            .add_table(TableConfig::new::<Charge>("charge"))
            .add_table(TableConfig::new::<Invoice>("invoice"));

        // set up invoice command
        context
            .add_command(Command::new("invoice")
                .alias("inv")
                .about("Invoice related commands")
                .subcommand(Command::new("generate")
                    .alias("gen")
                    .about("Generates an invoice document")
                    .arg(Arg::new("invoice-id")
                        .long("invoice-id")
                        .value_parser(clap::value_parser!(i64))
                        .required(true)
                        .help("The invoice row ID to \
                            generate a document from.")
                    )
                    .arg(Arg::new("out-dir")
                        .long("out-dir")
                        .value_parser(clap::value_parser!(PathBuf))
                        .required(true)
                        .help("The folder to output the document to")
                    )
                ),
                process_invoice_command
        )?;

        #[cfg(feature="tui")]
        if let Some(new_tab_types) = context.get_resource_mut::<tui::TuiNewTabTypes>() {
            new_tab_types.register_new_tab_type::<ExportInvoiceTabImpl>("Export Invoice");
        }

        context.add_new_window_type::<InvoiceExportWindow>("Export Invoice");

        Ok(())
    }
}

struct InvoiceExportWindow;

/// Processes the `generate` subcommand of the `invoice` command.
fn process_invoice_generate_command(
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> dolmen::Result<CommandResponse> {
    // get the command arguments
    let invoice_row_id = arg_matches
        .get_one::<i64>("invoice-id")
        .expect("Missing required argument");
    let out_folder = arg_matches
        .get_one::<PathBuf>("out-dir")
        .expect("Missing required argument");

    // create and export the invoice
    create_invoice(
        db_connection,
        out_folder.clone(),
        RowId(*invoice_row_id),
    )?;

    // return the command response
    Ok(CommandResponse::new(format!(
        "Successfully generated invoice at {}.",
        out_folder.join("invoice.pdf").display()
    )))
}

/// Processes the main `invoice` command.
fn process_invoice_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> dolmen::Result<CommandResponse> {
    // get the database connection
    let db_connection =
        context.db_connection()?;

    // check for the generate subcommand and run it if desired
    if let Some(("generate", sub_m)) =
        arg_matches.subcommand()
    {
        return process_invoice_generate_command(
            sub_m,
            db_connection,
        );
    }

    Err(dolmen::Error::new(format!("subcommand not recognized")))
}

/// A LaTeX preamble element to create a new command. This is used to pass
/// data into the generated document.
struct NewCommand(String, String);

impl From<NewCommand> for PreambleElement {
    fn from(val: NewCommand) -> Self {
        PreambleElement::UserDefined(format!(
            "\\newcommand{{\\{}}}{{{}}}",
            val.0, val.1
        ))
    }
}

/// Creates a PDF document from an invoice.
///
/// * `db_connection` - A connection to the database.
/// * `out_path` - The directory to output the document to.
/// * `invoice_row_id` - The row ID in the `invoice` table corresponding to
///   the invoice to generate.
fn create_invoice(
    db_connection: &mut DbConnection,
    out_path: PathBuf,
    invoice_row_id: RowId,
) -> dolmen::Result<()> {
    // generate the LaTeX document
    let doc =
        generate_latex(db_connection, invoice_row_id)?;

    // export the PDF
    write_document(
        out_path.as_path(),
        "invoice",
        &doc,
    )
    .expect("failed to write document");

    Ok(())
}

/// Generates a LaTeX document from an invoice.
///
/// * `db_connection` - A connection to the database.
/// * `invoice_row_id` - The row ID in the `invoice` table corresponding to
///   the invoice to generate.
fn generate_latex(
    db_connection: &mut DbConnection,
    invoice_row_id: RowId,
) -> dolmen::Result<Document> {
    // get the relevant rows from the database
    let invoice = Invoice::from_table_row(
        db_connection,
        "invoice".into(),
        invoice_row_id,
    )?;
    let trainer = Trainer::from_table_row(
        db_connection,
        "trainer".into(),
        invoice.trainer,
    )?;
    let client = Client::from_table_row(
        db_connection,
        "client".into(),
        invoice.client,
    )?;

    let charges = invoice.charges.iter().map(|c| {
        // todo: get rid of this unwrap
        Charge::from_table_row(db_connection, "charge".into(), *c).unwrap() 
    }).collect::<Vec<_>>();

    // Create the document and set up the preamble with all the needed data.
    let mut doc =
        Document::new(DocumentClass::Article);
    doc.preamble.use_package("hhline");
    doc.preamble.push(PreambleElement::UsePackage {
        package: "geometry".into(),
        argument: Some("margin=0.5in".into()),
    });
    /*doc.preamble.push(PreambleElement::UsePackage {
        package: "quattrocento".into(),
        argument: Some("sfdefault".into()),
    });*/
    doc.preamble.push(PreambleElement::UsePackage {
        package: "fontenc".into(),
        argument: Some("T1".into()),
    });
    doc.preamble.push(PreambleElement::UsePackage {
        package: "graphicx".into(),
        argument: None
    });
    doc.preamble.push(PreambleElement::UsePackage {
        package: "array".into(),
        argument: None
    });
    doc.preamble.push(NewCommand(
        "trainername".into(),
        trainer.name().clone(),
    ));
    doc.preamble.push(NewCommand(
        "companyname".into(),
        trainer.company_name().clone(),
    ));
    doc.preamble.push(NewCommand(
        "companyaddress".into(),
        trainer.address().clone(),
    ));
    doc.preamble.push(NewCommand(
        "companyemail".into(),
        trainer.email().clone(),
    ));
    doc.preamble.push(NewCommand(
        "companyphone".into(),
        trainer.phone().clone(),
    ));
    doc.preamble.push(NewCommand(
        "clientname".into(),
        client.name().clone(),
    ));
    doc.preamble.push(NewCommand(
        "invoicenumber".into(),
        invoice.invoice_number,
    ));
    doc.preamble.push(NewCommand(
        "paymentdue".into(),
        invoice.due_date,
    ));
    doc.preamble.push(NewCommand(
        "paymentmade".into(),
        invoice.date_paid,
    ));
    doc.preamble.push(NewCommand(
        "paidvia".into(),
        invoice.paid_via,
    ));

    let mut charge_data = String::from("");
    let mut charge_total = 0;

    for charge in charges {
        charge_data.push_str(format!("{} & {} & {} \\\\", charge.date, charge.description, charge.amount).as_str());
        charge_total += charge.amount;
    }

    doc.preamble.push(NewCommand(
        "chargedata".into(),
        charge_data,
    ));

    doc.preamble.push(NewCommand(
        "chargetotal".into(),
        format!("{}", charge_total),
    ));

    let company_header = if let Some(logo_path) = trainer.logo_path() {
        format!("\\includegraphics[width=256px]{{{}}}", logo_path)
    } else {
        format!("\\Large\\textbf{{{}}}", trainer.company_name())
    };
    // TODO: get rid of this unwrap
    doc.preamble.push(NewCommand(
        "companyheader".into(),
        company_header
    ));

    // push the invoice template into the document now that all commands are
    // set
    doc.push(Element::UserDefined(
        include_str!("invoice_template.tex").into(),
    ));

    Ok(doc)
}

// TODO: implement this
#[cfg(feature="tui")]
struct ExportInvoiceTabImpl;

#[cfg(feature="tui")]
#[derive(Default)]
struct ExportInvoiceTabState;

#[cfg(feature="tui")]
impl TabImpl for ExportInvoiceTabImpl {
    type State = ExportInvoiceTabState;

    fn title() -> String { "Export Invoice".into() }

    fn render(_: &mut Context, buffer: &mut Buffer, rect: Rect, block: Block, _: usize) {
        Paragraph::new(Line::from("Export Invoice not implemented.")).block(block).render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        vec!()
    }

    fn handle_key(_: &mut Context, _: &str, _: usize) {

    }

    fn handle_text(_: &mut Context, _: ratatui::crossterm::event::Event, _: usize) {

    }
}

#[cfg(test)]
mod test {
    use crate::InvoicePlugin;
    use dolmen::prelude::*;
    use framework::prelude::*;
    use training::TrainingPlugin;

    fn setup_invoice_data(
        db_connection: &mut DbConnection,
    ) -> dolmen::Result<RowId> {
        // TODO: setting each field like this is super verbose and not
        // typesafe, this should be wrapped
        let client = db_connection
            .new_row_in_table("client")?;
        db_connection.set_field_in_table(
            "client",
            client,
            "name",
            "Clarissa Client",
        )?;

        let trainer = db_connection
            .new_row_in_table("trainer")?;
        db_connection.set_field_in_table(
            "trainer",
            trainer,
            "name",
            "Tara Trainer",
        )?;
        db_connection.set_field_in_table(
            "trainer",
            trainer,
            "company_name",
            "Tara Fitness",
        )?;
        db_connection.set_field_in_table(
            "trainer",
            trainer,
            "address",
            "2127 Xanthia St, Denver, CO 80220",
        )?;
        db_connection.set_field_in_table(
            "trainer",
            trainer,
            "email",
            "tara@gmail.com",
        )?;
        db_connection.set_field_in_table(
            "trainer",
            trainer,
            "phone",
            "(303) 175-3098",
        )?;

        let charge = db_connection
            .new_row_in_table("charge")?;
        db_connection.set_field_in_table(
            "charge",
            charge,
            "date",
            "11/05/2025",
        )?;
        db_connection.set_field_in_table(
            "charge",
            charge,
            "description",
            "Personal training session (60 min)",
        )?;
        db_connection.set_field_in_table(
            "charge", charge, "amount", "50",
        )?;

        let invoice = db_connection
            .new_row_in_table("invoice")?;
        // TODO: I don't like passing client.0 here, RowId should implement
        // ToSql
        db_connection.set_field_in_table(
            "invoice", invoice, "client", client.0,
        )?;
        db_connection.set_field_in_table(
            "invoice", invoice, "trainer", trainer.0,
        )?;
        db_connection.set_field_in_table(
            "invoice",
            invoice,
            "invoice_number",
            "2025-0532",
        )?;
        db_connection.set_field_in_table(
            "invoice",
            invoice,
            "due_date",
            "11/06/2025",
        )?;
        db_connection.set_field_in_table(
            "invoice",
            invoice,
            "date_paid",
            "11/07/2025",
        )?;
        db_connection.set_field_in_table(
            "invoice", invoice, "paid_via", "Cash",
        )?;
        // TODO: same here, Vec<RowId> should implement ToSql
        db_connection.set_field_in_table(
            "invoice",
            invoice,
            "charges",
            format!("{}", charge.0),
        )?;

        

        Ok(invoice)
    }

    #[test]
    fn invoice_generate_test() -> dolmen::Result<()> {
        let mut context = Context::new();
        context
            .add_plugin(DbPlugin)?
            .add_plugin(InvoicePlugin)?
            .add_plugin(TrainingPlugin)?;
        context.get_resource_mut::<DbConfig>().unwrap().open_db_in_memory = true;

        context.startup()?;
        let db_connection = context.db_connection()?;

        let invoice =
            setup_invoice_data(db_connection)?;

        let latex = crate::generate_latex(
            db_connection,
            invoice,
        )?;

        let rendered = latex::print(&latex).unwrap();

        insta::assert_snapshot!(rendered);

        let out_path = std::env::temp_dir()
            .join("training_assistant_test");

        std::fs::create_dir_all(out_path.clone()).unwrap();

        documents::write_document(
            out_path.as_path(),
            "invoice",
            &latex,
        )
        .expect("failed to write document");

        assert!(std::fs::exists(
            out_path.join("invoice.pdf")
        ).unwrap());

        std::fs::remove_dir_all(out_path).unwrap();

        Ok(())
    }

    #[test]
    fn test_billing_commands() -> dolmen::Result<()> {
        let mut context = Context::new();
        context
            .add_plugin(DbPlugin)?
            .add_plugin(InvoicePlugin)?
            .add_plugin(TrainingPlugin)?;
        context.get_resource_mut::<DbConfig>().unwrap().open_db_in_memory = true;

        context.startup()?;

        let db_connection = context.db_connection()?;

        let invoice =
            setup_invoice_data(db_connection)?;

        let latex = crate::generate_latex(
            db_connection,
            invoice,
        )?;

        let out_path = std::env::temp_dir();
        println!("{:?}", out_path);

        std::fs::create_dir_all(out_path.clone()).unwrap();

        documents::write_document(
            out_path.as_path(),
            "invoice",
            &latex,
        )
        .expect("failed to write document");

        let response = context.execute(
            format!("invoice generate --invoice-id={} --out-dir={}", invoice.0, 
                out_path.as_os_str().to_str().unwrap()
            ).as_str()
        )?;

        assert!(response.text().is_some());
        assert_eq!(
            response.text().unwrap(),
            "Successfully generated invoice at /tmp/invoice.pdf."
        );

        Ok(())
    }
}
