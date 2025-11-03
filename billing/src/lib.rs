use std::path::PathBuf;
use latex::{Document, DocumentClass, Element, PreambleElement};
use documents::write_document;
use framework::prelude::*;
use clap::{Command, Arg, ArgMatches};
use framework::Result;
use rusqlite::Connection;
use framework_derive_macros::TableRow;

#[derive(Default, Clone)]
pub struct InvoicePlugin;

impl Plugin for InvoicePlugin {
    fn build(self, context: &mut Context) {
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
                process_invoice_command)
            .add_table::<Charge>("charge".into())
            .add_table::<Invoice>("invoice".into());
    }
}

fn process_invoice_generate_command(arg_matches: &ArgMatches, db_connection: &mut DatabaseConnection) {
    let invoice_row_id = arg_matches.get_one::<i64>("invoice-id").expect("Missing required argument");
    let out_folder = arg_matches.get_one::<PathBuf>("out-folder").expect("Missing required argument");

    create_invoice(db_connection, out_folder.clone(), RowId(*invoice_row_id));
}

fn process_invoice_command(arg_matches: &ArgMatches, db_connection: &mut DatabaseConnection) -> Result<CommandResponse> { 
    if let Some(("generate", sub_m)) = arg_matches.subcommand() {process_invoice_generate_command(sub_m, db_connection)}

    Ok(CommandResponse::default())
}

struct NewCommand(String, String);

impl From<NewCommand> for PreambleElement {
    fn from(val: NewCommand) -> Self {
        PreambleElement::UserDefined(format!("\\newcommand{{\\{}}}{{{}}}", val.0, val.1)) 
    }
}

pub fn create_invoice(
    db_connection: &mut DatabaseConnection, 
    out_path: PathBuf, 
    invoice_row_id: RowId,
) {
    let invoice = Invoice::from_table_row(db_connection, "invoice".into(), invoice_row_id).unwrap();
    let trainer = Trainer::from_table_row(db_connection, "trainer".into(), invoice.trainer).unwrap();
    let client = Client::from_table_row(db_connection, "client".into(), invoice.client).unwrap();
    let charge = Charge::from_table_row(db_connection, "charge".into(), invoice.charges[0]).unwrap();

    println!("creating invoice at {:?}", out_path);
    let mut doc = Document::new(DocumentClass::Article);
    doc.preamble.use_package("hhline");
    doc.preamble.push(PreambleElement::UsePackage { package: "geometry".into(), argument: Some("margin=0.5in".into()) });
    doc.preamble.push(NewCommand("companyname".into(), trainer.company_name));
    doc.preamble.push(NewCommand("companyaddress".into(), trainer.address));
    doc.preamble.push(NewCommand("companyemail".into(), trainer.email));
    doc.preamble.push(NewCommand("companyphone".into(), trainer.phone));
    doc.preamble.push(NewCommand("clientname".into(), client.name));
    doc.preamble.push(NewCommand("invoicenumber".into(), invoice.invoice_number));
    doc.preamble.push(NewCommand("paymentdue".into(), invoice.due_date));
    doc.preamble.push(NewCommand("paymentmade".into(), invoice.date_paid));
    doc.preamble.push(NewCommand("paidvia".into(), invoice.paid_via));
    doc.preamble.push(NewCommand("chargedate".into(), charge.date));
    doc.preamble.push(NewCommand("chargedescription".into(), charge.description));
    doc.preamble.push(NewCommand("chargeamount".into(), charge.amount.to_string()));

    doc.push(Element::UserDefined(include_str!("invoice_template.tex").into()));

    write_document(out_path.as_path(), "invoice", &doc).expect("failed to write document");
}

#[derive(TableRow)]
pub struct Invoice {
    pub client: RowId,
    pub trainer: RowId,
    pub invoice_number: String,
    pub due_date: String,
    pub date_paid: String,
    pub paid_via: String,
    pub charges: Vec<RowId>
}


#[derive(TableRow)]
pub struct Charge {
    pub date: String,
    pub description: String,
    pub amount: i32
}

