use std::path::PathBuf;
use latex::{Document, DocumentClass, Element, PreambleElement};
use documents::write_document;
use db::{DatabaseConnection, TrainerId, ClientId, RowId};

struct NewCommand(String, String);

impl Into<PreambleElement> for NewCommand {
    fn into(self) -> PreambleElement {
        PreambleElement::UserDefined(format!("\\newcommand{{\\{}}}{{{}}}", self.0, self.1)) 
    }
}

pub fn create_invoice(db_connection: &mut DatabaseConnection, out_path: PathBuf, invoice_row_id: RowId, trainer_row_id: RowId, client_row_id: RowId){
    /*let Ok(trainer_metadata) = db_connection.get_trainer_metadata(trainer_id) else {
        println!("couldn't get trainer metadata");
        return;
    };

    let Ok(client_metadata) = db_connection.get_client_metadata(client_id) else {
        println!("couldn't get client metadata");
        return;
    }; */

    let company_name = db_connection.get_field_in_table_row::<String>("trainers".into(), trainer_row_id, "companyname".into()).unwrap();
    let company_address = db_connection.get_field_in_table_row::<String>("trainers".into(), trainer_row_id, "address".into()).unwrap();
    let company_email = db_connection.get_field_in_table_row::<String>("trainers".into(), trainer_row_id, "email".into()).unwrap();
    let company_phone = db_connection.get_field_in_table_row::<String>("trainers".into(), trainer_row_id, "phone".into()).unwrap();
    let client_name = db_connection.get_field_in_table_row::<String>("clients".into(), client_row_id, "name".into()).unwrap();
    let invoice_number = db_connection.get_field_in_table_row::<String>("invoices".into(), invoice_row_id, "invoice_number".into()).unwrap();
    let payment_due = db_connection.get_field_in_table_row::<String>("invoices".into(), invoice_row_id, "due_date".into()).unwrap();
    let payment_made = db_connection.get_field_in_table_row::<String>("invoices".into(), invoice_row_id, "date_paid".into()).unwrap();
    let paid_via = db_connection.get_field_in_table_row::<String>("invoices".into(), invoice_row_id, "paid_via".into()).unwrap();
    let charges = db_connection.get_field_in_table_row::<String>("invoices".into(), invoice_row_id, "charges".into()).unwrap();
    let charge_row_ids: Vec<_> = charges.split(',').collect();
    
    let charge_first_row_id = charge_row_ids[0].parse::<i64>().unwrap();

    let charge_date = db_connection.get_field_in_table_row::<String>("charges".into(), RowId(charge_first_row_id), "date".into()).unwrap();
    let charge_description = db_connection.get_field_in_table_row::<String>("charges".into(), RowId(charge_first_row_id), "description".into()).unwrap();
    let charge_amount = db_connection.get_field_in_table_row::<i32>("charges".into(), RowId(charge_first_row_id), "amount".into()).unwrap();

    println!("creating invoice at {:?}", out_path);
    let mut doc = Document::new(DocumentClass::Article);
    doc.preamble.use_package("hhline");
    doc.preamble.push(PreambleElement::UsePackage { package: "geometry".into(), argument: Some("margin=0.5in".into()) });
    doc.preamble.push(NewCommand("companyname".into(), company_name));
    doc.preamble.push(NewCommand("companyaddress".into(), company_address));
    doc.preamble.push(NewCommand("companyemail".into(), company_email));
    doc.preamble.push(NewCommand("companyphone".into(), company_phone));
    doc.preamble.push(NewCommand("clientname".into(), client_name));
    doc.preamble.push(NewCommand("invoicenumber".into(), invoice_number));
    doc.preamble.push(NewCommand("paymentdue".into(), payment_due));
    doc.preamble.push(NewCommand("paymentmade".into(), payment_made));
    doc.preamble.push(NewCommand("paidvia".into(), paid_via));
    doc.preamble.push(NewCommand("chargedate".into(), charge_date));
    doc.preamble.push(NewCommand("chargedescription".into(), charge_description));
    doc.preamble.push(NewCommand("chargeamount".into(), charge_amount.to_string()));

    doc.push(Element::UserDefined(include_str!("invoice_template.tex").into()));

    write_document(out_path.as_path(), "invoice", &doc).expect("failed to write document");
}
