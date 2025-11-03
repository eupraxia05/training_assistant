use proc_macro::TokenStream;
use venial::{Fields, TypeExpr};
use quote::ToTokens;

fn type_expr_to_type_str(type_expr: &TypeExpr) -> String {
    let Some(path) = type_expr.as_path() else {
        panic!("couldn't expand TypeExpr path!");
    };
    let mut result = String::from("");
    for (idx, segment) in path.segments.iter().enumerate() {
        result += segment.ident.to_string().as_str();
        if idx != path.segments.len() - 1 {
            result += "::";
        }
        if let Some(generic_args) = &segment.generic_args {
            result += "::";
            result += "<";
            for generic_arg in generic_args.args.iter() {
                result += generic_arg.0.to_token_stream().to_string().as_str();
                result += ",";
            }
            result += ">";
            if idx != path.segments.len() - 1 {
                result += "::";
            }
        }
    }

    result
}

#[proc_macro_derive(Row)]
pub fn derive_row(item: TokenStream) -> TokenStream {
    let parsed_struct = match venial::parse_item(
        proc_macro2::TokenStream::from(item)
    ) {
        Ok(venial::Item::Struct(s)) => {s},
        Ok(_) => {panic!(concat!("Can only use ",
            "Row derive macro on structs"))},
        Err(_) => {panic!("Parsing error")}
    };

    let parsed_fields = match parsed_struct.fields {
        Fields::Named(named_fields) => {named_fields},
        _ => {panic!(concat!("Can only use Row ",
            "derive macro on structs with ",
            "named fields"))}
    };

    let struct_name = parsed_struct.name;

    let mut fields_setup_sql 
        = proc_macro2::TokenStream::new();

    for (idx, (field, _)) in parsed_fields.fields.iter().enumerate() {
        let mut s: String = format!(
            "table_setup_sql += \"{} \";",
            field.name.to_string().as_str(), 
        );
        s += "table_setup_sql += ";
        /*if field.ty.as_path().unwrap().segments.iter().any(|s| s.generic_args.is_some()) {
            panic!("{}", type_expr_to_type_str(&field.ty));
        }*/
        s += type_expr_to_type_str(&field.ty).as_str();
        s += "::sql_type();";
        if idx != parsed_fields.fields.len() - 1 {
           s += "table_setup_sql += \",\";";
        }
        fields_setup_sql.extend(
            s.parse::<proc_macro2::TokenStream>()
            .expect("failed to parse tokenstream 1")
        );
    }

    let setup_body = quote::quote!{
        let mut table_setup_sql: String = "CREATE TABLE IF NOT EXISTS ".into();
        table_setup_sql += table_name.as_str();
        table_setup_sql += "(id INTEGER PRIMARY KEY, ";
        #fields_setup_sql
        table_setup_sql += ");";
        connection.execute(table_setup_sql.as_str(), [])?;
        Ok(())
    };

    let mut from_table_row_body = proc_macro2::TokenStream::new();

    for (field, _) in parsed_fields.fields.iter() {
        let s: String = format!("let {} = {}::from_table_field(db_connection, table_name.clone(), row_id, \"{}\".into())?;",
            field.name.to_string().as_str(),
            type_expr_to_type_str(&field.ty),
            field.name.to_string().as_str()
        );
        from_table_row_body.extend(s.parse::<proc_macro2::TokenStream>().expect("fail 3"));
    }

    let mut self_str: String = "Ok(Self {".into();

    for (field, _) in parsed_fields.fields.iter() {
        self_str += field.name.to_string().as_str();
        self_str += ",";
    }
    self_str += "})";
    from_table_row_body.extend(self_str.parse::<proc_macro2::TokenStream>().unwrap());

    let generated = quote::quote!{
        impl RowType for #struct_name {
            fn setup(connection: &mut Connection, table_name: String) 
                -> Result<()> 
            {
                #setup_body
            }

            fn from_table_row(
                db_connection: &mut DatabaseConnection, 
                table_name: String,
                row_id: RowId
            ) -> Result<Self> 
            {
               #from_table_row_body 
            }
        }
    };

    proc_macro::TokenStream::from(generated)
}
