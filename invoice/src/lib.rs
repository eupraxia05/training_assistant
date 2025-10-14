use std::path::PathBuf;
use latex::{Document, DocumentClass, Element, PreambleElement};
use documents::write_document;
use training::{DatabaseConnection, TrainerId, ClientId};

struct NewCommand(String, String);

impl Into<PreambleElement> for NewCommand {
    fn into(self) -> PreambleElement {
        PreambleElement::UserDefined(format!("\\newcommand{{\\{}}}{{{}}}", self.0, self.1)) 
    }
}

pub fn create_invoice(db_connection: &mut DatabaseConnection, out_path: PathBuf, trainer_id: TrainerId, client_id: ClientId){
    let Ok(trainer_metadata) = db_connection.get_trainer_metadata(trainer_id) else {
        println!("couldn't get trainer metadata");
        return;
    };

    let Ok(client_metadata) = db_connection.get_client_metadata(client_id) else {
        println!("couldn't get client metadata");
        return;
    };

    println!("creating invoice at {:?}", out_path);
    let mut doc = Document::new(DocumentClass::Article);
    doc.preamble.use_package("hhline");
    doc.preamble.push(NewCommand("companyname".into(), trainer_metadata.company_name().into()));
    doc.preamble.push(NewCommand("companyaddress".into(), trainer_metadata.address().into()));
    doc.preamble.push(NewCommand("companyemail".into(), trainer_metadata.email().into()));
    doc.preamble.push(NewCommand("companyphone".into(), trainer_metadata.phone().into()));
    doc.preamble.push(NewCommand("clientname".into(), client_metadata.name().into()));

    doc.push(Element::UserDefined(include_str!("invoice_template.tex").into()));

    write_document(out_path.as_path(), "invoice", &doc).expect("failed to write document");
}
