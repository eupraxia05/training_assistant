//! A plugin for generating invoices and tracking charges.
use clap::{Arg, ArgMatches, Command};
use documents::write_document;
use framework::Result;
use framework::prelude::*;
use framework_derive_macros::TableRow;
use latex::{
    Document, DocumentClass, Element, PreambleElement,
};
use std::path::PathBuf;

/// A `Plugin` that sets up the required commands and
/// tables for billing.
#[derive(Default, Clone)]
pub struct InvoicePlugin;

impl Plugin for InvoicePlugin {
    fn build(self, context: &mut Context) {
        // set up charge and invoice tables
        context
            .add_table::<Charge>("charge")
            .add_table::<Invoice>("invoice");

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
                        .help("The invoice row ID to generate a document from.")
                    )
                    .arg(Arg::new("out-folder")
                        .long("out-folder")
                        .value_parser(clap::value_parser!(PathBuf))
                        .required(true)
                        .help("The folder to output the document to")
                    )
                ),
                process_invoice_command
            );
    }
}

fn process_invoice_generate_command(
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
    let invoice_row_id = arg_matches
        .get_one::<i64>("invoice-id")
        .expect("Missing required argument");
    let out_folder = arg_matches
        .get_one::<PathBuf>("out-folder")
        .expect("Missing required argument");

    create_invoice(
        db_connection,
        out_folder.clone(),
        RowId(*invoice_row_id),
    )?;

    Ok(CommandResponse::default())
}

fn process_invoice_command(
    context: &mut Context,
    arg_matches: &ArgMatches,
) -> Result<CommandResponse> {
    let db_connection = context.db_connection().unwrap();
    if let Some(("generate", sub_m)) =
        arg_matches.subcommand()
    {
        return process_invoice_generate_command(
            sub_m,
            db_connection,
        );
    }

    Ok(CommandResponse::default())
}

struct NewCommand(String, String);

impl From<NewCommand> for PreambleElement {
    fn from(val: NewCommand) -> Self {
        PreambleElement::UserDefined(format!(
            "\\newcommand{{\\{}}}{{{}}}",
            val.0, val.1
        ))
    }
}

pub fn create_invoice(
    db_connection: &mut DbConnection,
    out_path: PathBuf,
    invoice_row_id: RowId,
) -> Result<()> {
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
    let charge = Charge::from_table_row(
        db_connection,
        "charge".into(),
        invoice.charges[0],
    )?;

    let mut doc =
        Document::new(DocumentClass::Article);
    doc.preamble.use_package("hhline");
    doc.preamble.push(PreambleElement::UsePackage {
        package: "geometry".into(),
        argument: Some("margin=0.5in".into()),
    });
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
    doc.preamble.push(NewCommand(
        "chargedate".into(),
        charge.date,
    ));
    doc.preamble.push(NewCommand(
        "chargedescription".into(),
        charge.description,
    ));
    doc.preamble.push(NewCommand(
        "chargeamount".into(),
        charge.amount.to_string(),
    ));

    doc.push(Element::UserDefined(
        include_str!("invoice_template.tex").into(),
    ));

    write_document(
        out_path.as_path(),
        "invoice",
        &doc,
    )
    .expect("failed to write document");

    Ok(())
}

#[derive(TableRow, Debug)]
pub struct Invoice {
    pub client: RowId,
    pub trainer: RowId,
    pub invoice_number: String,
    pub due_date: String,
    pub date_paid: String,
    pub paid_via: String,
    pub charges: Vec<RowId>,
}

#[derive(TableRow, Debug)]
pub struct Charge {
    pub date: String,
    pub description: String,
    pub amount: i32,
}
