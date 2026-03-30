//! A plugin for generating invoices and tracking charges.
use clap::{Arg, ArgMatches, Command};
use documents::write_document;
use dolmen::prelude::*;
use latex::{
    Document, DocumentClass, Element, PreambleElement,
};
use reliquary::prelude::*;
use std::path::PathBuf;
use training::{Client, Trainer};

#[cfg(feature = "tui")]
use tui::{KeyBind, TabImpl};

use gui::prelude::*;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::{Block, Paragraph, Widget};

///////////////////////////////////////////////////////////////////////////////
// PUBLIC API
///////////////////////////////////////////////////////////////////////////////
/// A `Plugin` that sets up the required commands and
/// tables for billing.
#[derive(Default, Clone)]
pub struct BillingPlugin;

/// A table row storing a single issued charge. Stored in the table
/// `charge`.
#[derive(TableRow, Debug)]
pub struct Charge {
    /// The date the charge was issued.
    pub date: chrono::NaiveDate,

    /// A description of the charge
    /// (e.g. `"Personal training session (60 min)"`)
    pub description: String,

    /// The amount charged.
    // TODO: replace this with a proper currency field
    pub amount: i32,

    pub client: RowId,
}

#[derive(TableRow, Debug)]
pub struct Payment {
    pub date: chrono::NaiveDate,

    pub trainer: RowId,

    pub client: RowId,

    // TODO: replace this with a proper currency field
    pub amount: u32,

    pub paid_via: String,

    pub receipt_number: String,
}

///////////////////////////////////////////////////////////////////////////////
// PRIVATE IMPLEMENTATION
///////////////////////////////////////////////////////////////////////////////
impl Plugin for BillingPlugin {
    fn build(
        self,
        context: &mut Context,
    ) -> dolmen::Result<()> {
        // set up charge and invoice tables
        context
            .add_table(TableConfig::new::<Charge>(
                "charge",
            ))
            .add_table(TableConfig::new::<Payment>(
                "payment",
            ));

        // set up invoice command
        context
            .add_command(Command::new("invoice")
                .alias("inv")
                .about("Invoice related commands")
                .subcommand(Command::new("generate")
                    .alias("gen")
                    .about("Generates an invoice document")
                    .arg(Arg::new("payment-id")
                        .long("payment-id")
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
        .get_one::<i64>("payment-id")
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
    let db_connection = context.db_connection()?;

    // check for the generate subcommand and run it if desired
    if let Some(("generate", sub_m)) =
        arg_matches.subcommand()
    {
        return process_invoice_generate_command(
            sub_m,
            db_connection,
        );
    }

    Err(dolmen::Error::new(format!(
        "subcommand not recognized"
    )))
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

struct ReceiptInfo {
    // TODO: replace these with a proper currency field
    start_balance: i32,
    end_balance: i32,
    charges: Vec<RowId>,
    charge_total: u32,
    last_payment_date: String,
}

fn get_receipt_info(
    db_connection: &mut DbConnection,
    payment_row_id: RowId,
) -> dolmen::Result<ReceiptInfo> {
    let payment = Payment::from_table_row(
        db_connection,
        "payment".into(),
        payment_row_id,
    )?;

    let payment_row_ids =
        db_connection.get_table_row_ids("payment")?;

    let charge_row_ids =
        db_connection.get_table_row_ids("charge")?;

    let payments_for_client = payment_row_ids
        .iter()
        .filter(|p| {
            **p != payment_row_id.0
                && db_connection
                    .get_field_in_table_row::<String>(
                        "payment",
                        RowId(**p),
                        "date",
                    )
                    .unwrap()
                    < payment.date.to_string()
                && db_connection
                    .get_field_in_table_row::<RowId>(
                        "payment",
                        RowId(**p),
                        "client",
                    )
                    .unwrap()
                    == payment.client
        })
        .collect::<Vec<_>>();

    // get last payment date
    let last_payment_date = payments_for_client
        .iter()
        .map(|p| {
            db_connection
                .get_field_in_table_row::<String>(
                    "payment",
                    RowId(**p),
                    "date",
                )
                .unwrap()
        })
        .max()
        .unwrap_or("0000-01-01".into());

    let charges_for_client = charge_row_ids
        .iter()
        .filter(|c| {
            db_connection
                .get_field_in_table_row::<RowId>(
                    "charge",
                    RowId(**c),
                    "client",
                )
                .unwrap()
                == payment.client
        })
        .collect::<Vec<_>>();

    let charges_for_last_receipt = charges_for_client
        .iter()
        .filter(|c| {
            db_connection
                .get_field_in_table_row::<String>(
                    "charge",
                    RowId(***c),
                    "date",
                )
                .unwrap()
                <= last_payment_date
        })
        .collect::<Vec<_>>();

    let charges_for_this_receipt = charges_for_client
        .iter()
        .filter(|c| {
            let charge_date = db_connection
                .get_field_in_table_row::<String>(
                    "charge",
                    RowId(***c),
                    "date",
                )
                .unwrap();
            charge_date > last_payment_date
                && charge_date
                    <= payment.date.to_string()
        })
        .collect::<Vec<_>>();

    let mut payment_total = 0;
    for payment in payments_for_client.iter() {
        payment_total += db_connection
            .get_field_in_table_row::<u32>(
                "payment",
                RowId(**payment),
                "amount",
            )
            .unwrap()
    }

    let mut last_receipt_charge_total = 0;
    for charge in charges_for_last_receipt.iter() {
        last_receipt_charge_total += db_connection
            .get_field_in_table_row::<u32>(
                "charge",
                RowId(***charge),
                "amount",
            )
            .unwrap()
    }

    let start_balance = last_receipt_charge_total
        as i32
        - payment_total as i32;

    let mut charge_total = 0;
    charges_for_this_receipt.iter().for_each(|c| {
        charge_total += db_connection
            .get_field_in_table_row::<u32>(
                "charge",
                RowId(***c),
                "amount",
            )
            .unwrap();
    });

    let end_balance = start_balance as i32
        + charge_total as i32
        - payment.amount as i32;

    Ok(ReceiptInfo {
        start_balance,
        end_balance,
        charges: charges_for_this_receipt
            .iter()
            .map(|c| RowId(***c))
            .collect::<Vec<_>>(),
        charge_total,
        last_payment_date,
    })
}

/// Generates a LaTeX document from an invoice.
///
/// * `db_connection` - A connection to the database.
/// * `invoice_row_id` - The row ID in the `invoice` table corresponding to
///   the invoice to generate.
fn generate_latex(
    db_connection: &mut DbConnection,
    payment_row_id: RowId,
) -> dolmen::Result<Document> {
    let payment = Payment::from_table_row(
        db_connection,
        "payment".into(),
        payment_row_id,
    )?;
    // get the relevant rows from the database
    let trainer = Trainer::from_table_row(
        db_connection,
        "trainer".into(),
        payment.trainer,
    )?;
    let client = Client::from_table_row(
        db_connection,
        "client".into(),
        payment.client,
    )?;

    let receipt_info = get_receipt_info(
        db_connection,
        payment_row_id,
    )
    .unwrap();

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
        argument: None,
    });
    doc.preamble.push(PreambleElement::UsePackage {
        package: "array".into(),
        argument: None,
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
        payment.receipt_number,
    ));
    doc.preamble.push(NewCommand(
        "paymentmade".into(),
        payment.date.to_string(),
    ));
    doc.preamble.push(NewCommand(
        "paidvia".into(),
        payment.paid_via,
    ));
    doc.preamble.push(NewCommand(
        "lastpayment".into(),
        receipt_info.last_payment_date,
    ));
    doc.preamble.push(NewCommand(
        "subtotal".into(),
        format!(
            "{}",
            receipt_info.charge_total as i32
                + receipt_info.start_balance
        ),
    ));

    let mut charge_data = String::new();
    receipt_info.charges.iter().for_each(|c| {
        let charge = Charge::from_table_row(
            db_connection,
            "charge".into(),
            *c,
        )
        .unwrap();
        charge_data += format!(
            "{} & {} & {} \\\\ ",
            charge.date,
            charge.description,
            charge.amount
        )
        .as_str();
    });

    doc.preamble.push(NewCommand(
        "chargedata".into(),
        charge_data,
    ));

    doc.preamble.push(NewCommand(
        "paymentamount".into(),
        format!("{}", payment.amount),
    ));

    doc.preamble.push(NewCommand(
        "balancestart".into(),
        format!("{}", receipt_info.start_balance),
    ));

    doc.preamble.push(NewCommand(
        "balanceend".into(),
        format!("{}", receipt_info.end_balance),
    ));

    let company_header =
        if let Some(logo_path) = trainer.logo_path() {
            format!(
                "\\includegraphics[width=256px]{{{}}}",
                logo_path
            )
        } else {
            format!(
                "\\Large\\textbf{{{}}}",
                trainer.company_name()
            )
        };
    // TODO: get rid of this unwrap
    doc.preamble.push(NewCommand(
        "companyheader".into(),
        company_header,
    ));

    // push the invoice template into the document now that all commands are
    // set
    doc.push(Element::UserDefined(
        include_str!("invoice_template.tex").into(),
    ));

    Ok(doc)
}

// TODO: implement this
#[cfg(feature = "tui")]
struct ExportInvoiceTabImpl;

#[cfg(feature = "tui")]
#[derive(Default)]
struct ExportInvoiceTabState;

#[cfg(feature = "tui")]
impl TabImpl for ExportInvoiceTabImpl {
    type State = ExportInvoiceTabState;

    fn title() -> String {
        "Export Invoice".into()
    }

    fn render(
        _: &mut Context,
        buffer: &mut Buffer,
        rect: Rect,
        block: Block,
        _: usize,
    ) {
        Paragraph::new(Line::from(
            "Export Invoice not implemented.",
        ))
        .block(block)
        .render(rect, buffer);
    }

    fn keybinds() -> Vec<KeyBind> {
        vec![]
    }

    fn handle_key(_: &mut Context, _: &str, _: usize) {
    }

    fn handle_text(
        _: &mut Context,
        _: ratatui::crossterm::event::Event,
        _: usize,
    ) {
    }
}

#[cfg(test)]
mod test {
    use crate::{BillingPlugin, get_receipt_info};
    use dolmen::prelude::*;
    use reliquary::prelude::*;
    use training::TrainingPlugin;

    fn add_test_trainer(
        db_connection: &mut DbConnection,
    ) -> dolmen::Result<RowId> {
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

        Ok(trainer)
    }

    fn add_test_client(
        db_connection: &mut DbConnection,
        name: &str,
    ) -> dolmen::Result<RowId> {
        // TODO: setting each field like this is super verbose and not
        // typesafe, this should be wrapped
        let client = db_connection
            .new_row_in_table("client")?;
        db_connection.set_field_in_table(
            "client", client, "name", name,
        )?;

        Ok(client)
    }

    fn add_test_charge(
        db_connection: &mut DbConnection,
        date: &str,
        amount: u32,
        client: RowId,
    ) -> dolmen::Result<RowId> {
        let charge = db_connection
            .new_row_in_table("charge")?;
        db_connection.set_field_in_table(
            "charge", charge, "date", date,
        )?;
        db_connection.set_field_in_table(
            "charge",
            charge,
            "description",
            "Personal training session (60 min)",
        )?;
        db_connection.set_field_in_table(
            "charge", charge, "amount", amount,
        )?;
        db_connection.set_field_in_table(
            "charge", charge, "client", client.0,
        )?;

        Ok(charge)
    }

    fn add_test_payment(
        db_connection: &mut DbConnection,
        client: RowId,
        trainer: RowId,
        date: String,
        amount: u32,
    ) -> dolmen::Result<RowId> {
        let payment = db_connection
            .new_row_in_table("payment")?;
        // TODO: I don't like passing client.0 here, RowId should implement
        // ToSql
        db_connection.set_field_in_table(
            "payment", payment, "client", client.0,
        )?;
        db_connection.set_field_in_table(
            "payment", payment, "trainer", trainer.0,
        )?;
        db_connection.set_field_in_table(
            "payment",
            payment,
            "receipt_number",
            "2025-0532",
        )?;
        db_connection.set_field_in_table(
            "payment", payment, "amount", amount,
        )?;
        db_connection.set_field_in_table(
            "payment", payment, "date", date,
        )?;
        db_connection.set_field_in_table(
            "payment", payment, "paid_via", "Cash",
        )?;

        Ok(payment)
    }

    fn setup_invoice_data(
        db_connection: &mut DbConnection,
    ) -> dolmen::Result<RowId> {
        let trainer = add_test_trainer(db_connection)?;
        let client = add_test_client(
            db_connection,
            "Clarissa Client",
        )?;
        let _charge = add_test_charge(
            db_connection,
            "2026-01-04",
            50,
            client,
        )?;
        let payment = add_test_payment(
            db_connection,
            client,
            trainer,
            "2026-01-04".into(),
            50,
        )?;

        Ok(payment)
    }

    #[test]
    fn invoice_generate_test() -> dolmen::Result<()> {
        let mut context = Context::new();
        context
            .add_plugin(DbPlugin)?
            .add_plugin(BillingPlugin)?
            .add_plugin(TrainingPlugin)?;
        context
            .get_resource_mut::<DbConfig>()
            .unwrap()
            .open_db_in_memory = true;

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

        std::fs::create_dir_all(out_path.clone())
            .unwrap();

        documents::write_document(
            out_path.as_path(),
            "invoice",
            &latex,
        )
        .expect("failed to write document");

        assert!(
            std::fs::exists(
                out_path.join("invoice.pdf")
            )
            .unwrap()
        );

        std::fs::remove_dir_all(out_path).unwrap();

        Ok(())
    }

    #[test]
    fn test_billing_commands() -> dolmen::Result<()> {
        let mut context = Context::new();
        context
            .add_plugin(DbPlugin)?
            .add_plugin(BillingPlugin)?
            .add_plugin(TrainingPlugin)?;
        context
            .get_resource_mut::<DbConfig>()
            .unwrap()
            .open_db_in_memory = true;

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

        std::fs::create_dir_all(out_path.clone())
            .unwrap();

        documents::write_document(
            out_path.as_path(),
            "invoice",
            &latex,
        )
        .expect("failed to write document");

        let response = context.execute(
            format!("invoice generate --payment-id={} --out-dir={}", invoice.0, 
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

    fn setup_test_context() -> dolmen::Result<Context>
    {
        let mut context = Context::new();

        context
            .add_plugin(DbPlugin)?
            .add_plugin(BillingPlugin)?
            .add_plugin(TrainingPlugin)?;

        context
            .get_resource_mut::<DbConfig>()
            .unwrap()
            .open_db_in_memory = true;

        context.startup()?;

        Ok(context)
    }

    // Simplest test case: one client, one charge, one payment. Same date, for 50.
    // We expect a start balance of 0, end balance of 0, and one relevant charge totaling 50.
    #[test]
    fn test_receipt_info_1() -> dolmen::Result<()> {
        let mut context = setup_test_context()?;

        let db_connection = context.db_connection()?;

        let trainer = add_test_trainer(db_connection)?;
        let client = add_test_client(
            db_connection,
            "Clarissa Client",
        )?;
        let charge = add_test_charge(
            db_connection,
            "2026-01-14",
            50,
            client,
        )?;
        let payment = add_test_payment(
            db_connection,
            client,
            trainer,
            "2026-01-14".into(),
            50,
        )?;

        let receipt_info =
            get_receipt_info(db_connection, payment)?;
        assert_eq!(receipt_info.charges.len(), 1);
        assert!(
            receipt_info.charges.contains(&charge)
        );
        assert_eq!(receipt_info.start_balance, 0);
        assert_eq!(receipt_info.end_balance, 0);
        assert_eq!(receipt_info.charge_total, 50);

        Ok(())
    }

    // One client, two charges, amount of 50 for each, payment for 90.
    // Expect a start balance of 0, end balance of 10, and two relevant charges totaling 100.
    #[test]
    fn test_receipt_info_2() -> dolmen::Result<()> {
        let mut context = setup_test_context()?;
        let db_connection = context.db_connection()?;
        let trainer = add_test_trainer(db_connection)?;
        let client = add_test_client(
            db_connection,
            "Clarissa Client",
        )?;
        let charge_1 = add_test_charge(
            db_connection,
            "2026-02-10",
            50,
            client,
        )?;
        let charge_2 = add_test_charge(
            db_connection,
            "2026-02-11",
            50,
            client,
        )?;
        let payment = add_test_payment(
            db_connection,
            client,
            trainer,
            "2026-02-12".into(),
            90,
        )?;

        let receipt_info =
            get_receipt_info(db_connection, payment)?;

        assert_eq!(receipt_info.start_balance, 0);
        assert_eq!(receipt_info.end_balance, 10);
        assert_eq!(receipt_info.charges.len(), 2);
        assert!(
            receipt_info.charges.contains(&charge_1)
        );
        assert!(
            receipt_info.charges.contains(&charge_2)
        );
        assert_eq!(receipt_info.charge_total, 100);

        Ok(())
    }

    // One client, three charges, two payments. All charges for 50. First payment for 60,
    // date after the first charge. Second payment for 90, date after the third charge.
    // For the first payment, expect start balance of 0, end balance of -10, only relevant
    // charge is the first, totaling 50.
    // For the second payment, expect start balance of -10, end balance of 0, relevant charges
    // are the second and third, totaling 100.
    #[test]
    fn test_receipt_info_3() -> dolmen::Result<()> {
        let mut context = setup_test_context()?;
        let db_connection = context.db_connection()?;
        let trainer = add_test_trainer(db_connection)?;
        let client = add_test_client(
            db_connection,
            "Clarissa Client",
        )?;
        let charge_1 = add_test_charge(
            db_connection,
            "2026-03-01",
            50,
            client,
        )?;
        let charge_2 = add_test_charge(
            db_connection,
            "2026-03-03",
            50,
            client,
        )?;
        let charge_3 = add_test_charge(
            db_connection,
            "2026-03-04",
            50,
            client,
        )?;
        let payment_1 = add_test_payment(
            db_connection,
            client,
            trainer,
            "2026-03-02".into(),
            60,
        )?;
        let payment_2 = add_test_payment(
            db_connection,
            client,
            trainer,
            "2026-03-05".into(),
            90,
        )?;

        let receipt_info_1 = get_receipt_info(
            db_connection,
            payment_1,
        )?;

        let receipt_info_2 = get_receipt_info(
            db_connection,
            payment_2,
        )?;

        assert_eq!(receipt_info_1.start_balance, 0);
        assert_eq!(receipt_info_1.end_balance, -10);
        assert_eq!(receipt_info_1.charges.len(), 1);
        assert!(
            receipt_info_1.charges.contains(&charge_1)
        );
        assert_eq!(receipt_info_1.charge_total, 50);

        assert_eq!(receipt_info_2.start_balance, -10);
        assert_eq!(receipt_info_2.end_balance, 0);
        assert_eq!(receipt_info_2.charges.len(), 2);
        assert!(
            receipt_info_2.charges.contains(&charge_2)
        );
        assert!(
            receipt_info_2.charges.contains(&charge_3)
        );
        assert_eq!(receipt_info_2.charge_total, 100);

        Ok(())
    }
}
