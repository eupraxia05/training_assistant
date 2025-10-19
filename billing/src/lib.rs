use std::path::PathBuf;
use latex::{Document, DocumentClass, Element, PreambleElement};
use documents::write_document;
use db::{DatabaseConnection, RowId, Client, RowType, Trainer, Invoice, Charge};

struct NewCommand(String, String);

impl Into<PreambleElement> for NewCommand {
    fn into(self) -> PreambleElement {
        PreambleElement::UserDefined(format!("\\newcommand{{\\{}}}{{{}}}", self.0, self.1)) 
    }
}

pub fn create_invoice(
    db_connection: &mut DatabaseConnection, 
    out_path: PathBuf, 
    invoice_row_id: RowId,
) {
    let invoice = Invoice::from_table_row(db_connection, invoice_row_id).unwrap();
    let trainer = Trainer::from_table_row(db_connection, invoice.trainer).unwrap();
    let client = Client::from_table_row(db_connection, invoice.client).unwrap();
    let charge = Charge::from_table_row(db_connection, invoice.charges[0]).unwrap();

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
