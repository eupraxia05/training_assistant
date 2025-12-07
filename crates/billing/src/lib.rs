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
use training::{Client, Trainer};
use tui::{TabImpl, KeyBind};
use ratatui::buffer::Buffer;
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{Block, Paragraph, Widget};
use ratatui::text::Line;

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
        );

        context.get_resource_mut::<tui::TuiNewTabTypes>().unwrap().register_new_tab_type::<ExportInvoiceTabImpl>("Export Invoice");
    }
}

/// Processes the `generate` subcommand of the `invoice` command.
fn process_invoice_generate_command(
    arg_matches: &ArgMatches,
    db_connection: &mut DbConnection,
) -> Result<CommandResponse> {
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
) -> Result<CommandResponse> {
    // get the database connection
    let db_connection =
        context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;

    // check for the generate subcommand and run it if desired
    if let Some(("generate", sub_m)) =
        arg_matches.subcommand()
    {
        return process_invoice_generate_command(
            sub_m,
            db_connection,
        );
    }

    // TODO: this isn't an unknown error, this should be a
    // "subcommand not recognized" error
    Err(Error::UnknownError)
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
) -> Result<()> {
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
) -> Result<Document> {
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
    let charge = Charge::from_table_row(
        db_connection,
        "charge".into(),
        invoice.charges[0],
    )?;

    // Create the document and set up the preamble with all the needed data.
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

    // push the invoice template into the document now that all commands are
    // set
    doc.push(Element::UserDefined(
        include_str!("invoice_template.tex").into(),
    ));

    Ok(doc)
}

// TODO: implement this
struct ExportInvoiceTabImpl;

#[derive(Default)]
struct ExportInvoiceTabState;

impl TabImpl for ExportInvoiceTabImpl {
    type State = ExportInvoiceTabState;

    fn title() -> String { "Export Invoice".into() }

    fn render(context: &mut Context, buffer: &mut Buffer, rect: Rect, block: Block, tab_id: usize) {
        Paragraph::new(Line::from("Export Invoice not implemented.")).block(block).render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        vec!()
    }

    fn handle_key(context: &mut Context, bind_name: &str, tab_idx: usize) {

    }
}

#[cfg(test)]
mod test {
    use crate::InvoicePlugin;
    use framework::prelude::*;
    use training::TrainingPlugin;

    fn setup_invoice_data(
        db_connection: &mut DbConnection,
    ) -> Result<RowId> {
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
    fn invoice_generate_test() -> Result<()> {
        let mut context = Context::new();
        context
            .add_plugin(DbPlugin)
            .add_plugin(InvoicePlugin)
            .add_plugin(TrainingPlugin);
        context.in_memory_db(true);

        context.startup()?;
        let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;

        let invoice =
            setup_invoice_data(db_connection)?;

        let latex = crate::generate_latex(
            db_connection,
            invoice,
        )?;

        let rendered = latex::print(&latex).unwrap();

        assert_eq!(
            rendered,
            "\\documentclass{article}\n\
            \\usepackage{hhline}\n\
            \\usepackage[margin=0.5in]{geometry}\n\
            \\newcommand{\\companyname}{Tara Fitness}\n\
            \\newcommand{\\companyaddress}\
                {2127 Xanthia St, Denver, CO 80220}\n\
            \\newcommand{\\companyemail}{tara@gmail.com}\n\
            \\newcommand{\\companyphone}{(303) 175-3098}\n\
            \\newcommand{\\clientname}{Clarissa Client}\n\
            \\newcommand{\\invoicenumber}{2025-0532}\n\
            \\newcommand{\\paymentdue}{11/06/2025}\n\
            \\newcommand{\\paymentmade}{11/07/2025}\n\
            \\newcommand{\\paidvia}{Cash}\n\
            \\newcommand{\\chargedate}{11/05/2025}\n\
            \\newcommand{\\chargedescription}\
                {Personal training session (60 min)}\n\
            \\newcommand{\\chargeamount}{50}\n\
            \\begin{document}\n\
            \\begin{center}\n\
            \t{\\Large\\bfseries \\companyname}\\\\\n\
            \t{\\companyaddress}\\\\\n\
            \tEmail: \\companyemail \\hspace{1cm} Phone \\companyphone\n\
            \\end{center}\n\n\
            \\vspace{0.5cm}\n\
            \\hrule\n\\vspace{0.5cm}\n\n\
            \\noindent{\\textbf{Invoice Number:} \\invoicenumber} \\\\\n\
            \\noindent{\\textbf{Client Name:} \\clientname} \\\\\n\
            \\noindent{\\textbf{Payment Due:} \\paymentdue} \\\\\n\
            \\noindent{\\textbf{Payment Made:} \\paymentmade} \\\\\n\
            \\noindent{\\textbf{Paid Via:} \\paidvia} \\\\\n\n\
            \\vspace{0.5cm}\n\
            \\begin{center}\n\
            \\begin{tabular}{|p{2.0cm}|p{8.0cm}|p{2.5cm}|}\n\
            \t\\hline\n\
            \t\\textbf{Date} & \\textbf{Description} \
                & \\textbf{Amount (\\$)} \\\\\n\
            \t\\hline\n\
            \t\\chargedate & \\chargedescription & \\chargeamount \\\\\n\
            \t\\hhline{|=|=|=|}\n\
            \t\\multicolumn{2}{|l|}{\\textbf{Total}} & \\textbf{100} \\\\\n\
            \t\\hline\n\
            \\end{tabular}\n\
            \\end{center}\n\n\
            \\vspace{0.5cm}\n\n\
            \\noindent{\\textit{Payment due at time of service. Refunds only \
                available for sessions cancelled at least \
                24 hours in advance.}\n\n\
            \\vspace{0.5cm}\n\n\
            \\noindent{\\textit{Thanks for training with me!}}\n\n\
            \\end{document}\n"
        );

        let out_path = std::env::temp_dir()
            .join("training_assistant_test");

        std::fs::create_dir_all(out_path.clone())?;

        documents::write_document(
            out_path.as_path(),
            "invoice",
            &latex,
        )
        .expect("failed to write document");

        assert!(std::fs::exists(
            out_path.join("invoice.pdf")
        )?);

        std::fs::remove_dir_all(out_path)?;

        Ok(())
    }

    #[test]
    fn test_billing_commands() -> Result<()> {
        let mut context = Context::new();
        context
            .add_plugin(DbPlugin)
            .add_plugin(InvoicePlugin)
            .add_plugin(TrainingPlugin);
        context.in_memory_db(true);

        context.startup()?;

        let db_connection = context.get_resource_mut::<DbConnection>().ok_or(Error::NoConnectionError)?;

        let invoice =
            setup_invoice_data(db_connection)?;

        let latex = crate::generate_latex(
            db_connection,
            invoice,
        )?;

        let out_path = std::env::temp_dir();
        println!("{:?}", out_path);

        std::fs::create_dir_all(out_path.clone())?;

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
