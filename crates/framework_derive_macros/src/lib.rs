//! Procedural macro definitions, of particular note
//! the `Row` macro which generates SQL table setup
//! code from a Rust struct.

use proc_macro::TokenStream;
use quote::ToTokens;
use venial::{Fields, TypeExpr};

/// Automatically implements the TableRow trait for
/// a given type, allowing a table to be created
/// from the type using `Context::add_table`.
#[proc_macro_derive(TableRow, attributes(display_table))]
pub fn derive_row(item: TokenStream) -> TokenStream {
    // parse the struct this derive macro was invoked
    // on (and panic if it's not a struct)
    let parsed_struct = match venial::parse_item(
        proc_macro2::TokenStream::from(item),
    ) {
        Ok(venial::Item::Struct(s)) => s,
        Ok(_) => {
            panic!(concat!(
                "Can only use ",
                "Row derive macro on structs"
            ))
        }
        Err(_) => {
            panic!("Parsing error")
        }
    };

    // parse the fields of the struct and panic if
    // it doesn't have named fields
    let parsed_fields = match &parsed_struct.fields {
        Fields::Named(named_fields) => named_fields,
        _ => {
            panic!(concat!(
                "Can only use Row ",
                "derive macro on structs with ",
                "named fields"
            ))
        }
    };

    let generated = generate_table_row_impl(
        &parsed_struct,
        &parsed_fields,
    );

    proc_macro::TokenStream::from(generated)
}

fn generate_table_row_impl(
    parsed_struct: &venial::Struct,
    parsed_fields: &venial::NamedFields,
) -> proc_macro2::TokenStream {
    let setup_fn_definition =
        generate_table_row_setup_fn_definition(
            parsed_fields,
        );
    let from_table_row_fn_definition 
        = generate_table_row_from_table_row_fn_definition(parsed_fields);
    let tabled_fn_definitions 
        = generate_tabled_fn_definitions(parsed_fields);
    let field_types_fn_definition 
        = generate_field_types_fn_definition(parsed_fields);
    let get_fields_as_strings_fn_definition 
        = generate_get_fields_as_strings_fn_definition(parsed_fields);

    let struct_name = parsed_struct.name.clone();

    quote::quote! {
        impl TableRow for #struct_name {
            #setup_fn_definition

            #from_table_row_fn_definition

            #tabled_fn_definitions

            #field_types_fn_definition

            #get_fields_as_strings_fn_definition
        }
    }
}

fn generate_table_row_setup_fn_definition(
    parsed_fields: &venial::NamedFields,
) -> proc_macro2::TokenStream {
    let mut fields_setup_sql =
        proc_macro2::TokenStream::new();

    for (idx, (field, _)) in
        parsed_fields.fields.iter().enumerate()
    {
        let mut s: String = format!(
            "table_setup_sql += \"{} \";",
            field.name.to_string().as_str(),
        );
        s += "table_setup_sql += ";
        s += type_expr_to_type_str(&field.ty).as_str();
        s += "::sql_type();";
        if idx != parsed_fields.fields.len() - 1 {
            s += "table_setup_sql += \",\";";
        }
        fields_setup_sql.extend(
            s.parse::<proc_macro2::TokenStream>()
                .expect(
                    "failed to parse tokenstream 1",
                ),
        );
    }

    quote::quote! {
        fn setup(connection: &mut rusqlite::Connection, table_name: String)
            -> framework::Result<()>
        {
            let mut table_setup_sql: String 
                = "CREATE TABLE IF NOT EXISTS ".into();
            table_setup_sql += table_name.as_str();
            table_setup_sql += "(id INTEGER PRIMARY KEY, ";
            #fields_setup_sql
            table_setup_sql += ");";
            connection.execute(table_setup_sql.as_str(), [])?;
            Ok(())
        }
    }
}

fn generate_table_row_from_table_row_fn_definition(
    parsed_fields: &venial::NamedFields,
) -> proc_macro2::TokenStream {
    let mut from_table_row_fn_body =
        proc_macro2::TokenStream::new();

    for (field, _) in parsed_fields.fields.iter() {
        let s: String = format!(
            "let {} = {}::from_table_field(db_connection, \
                table_name.clone(), row_id, \"{}\".into())?;",
            field.name.to_string().as_str(),
            type_expr_to_type_str(&field.ty),
            field.name.to_string().as_str()
        );
        from_table_row_fn_body.extend(
            s.parse::<proc_macro2::TokenStream>()
                .expect("fail 3"),
        );
    }

    let mut self_str: String = "Ok(Self {".into();

    for (field, _) in parsed_fields.fields.iter() {
        self_str += field.name.to_string().as_str();
        self_str += ",";
    }
    self_str += "})";
    from_table_row_fn_body.extend(
        self_str
            .parse::<proc_macro2::TokenStream>()
            .unwrap(),
    );

    quote::quote! {
        fn from_table_row(
            db_connection: &DbConnection,
            table_name: String,
            row_id: RowId
        ) -> framework::Result<Self>
        {
            #from_table_row_fn_body
        }
    }
}

fn generate_tabled_fn_definitions(fields: &venial::NamedFields) 
    -> proc_macro2::TokenStream 
{
    let mut fields_args = proc_macro2::TokenStream::new();

    for (field, _) in fields.fields.iter() {
        fields_args.extend(format!("\"{}\", ", 
            field.name.to_string()).parse::<proc_macro2::TokenStream>());
    }

    let mut record_fields_args = proc_macro2::TokenStream::new();
    for (field, _) in fields.fields.iter() {
        let field_str = format!("format!(\"{{:?}}\", record.{}), ", 
            field.name.to_string());
        record_fields_args.extend(
            field_str.parse::<proc_macro2::TokenStream>());
    }

    quote::quote!(
        fn push_tabled_header(builder: &mut tabled::builder::Builder) {
            builder.push_record(["ID", #fields_args]);
        }

        fn push_tabled_record(builder: &mut tabled::builder::Builder, 
            db_connection: &DbConnection, table_name: String, row_id: RowId) 
        {
            let Ok(record) 
                = Self::from_table_row(db_connection, table_name, row_id) 
            else {
                builder.push_record([row_id.0.to_string(), "Err".to_string()]);
                return;
            };
            println!("{:?}", record);
            builder.push_record([row_id.0.to_string(), #record_fields_args]);
        }
    )
}

fn generate_field_types_fn_definition(fields: &venial::NamedFields) 
    -> proc_macro2::TokenStream 
{
    let mut field_types = proc_macro2::TokenStream::new();

    for(field, _) in fields.fields.iter() {
        let field_name = field.name.to_string();
        let field_type = field.ty.clone();

        field_types.extend(quote::quote!{
            framework::db::FieldTypeInfo {
                name: #field_name.into(),
                type_id: std::any::TypeId::of::<#field_type>()
            },
        });
    }

    quote::quote!(
        fn field_types() -> Vec<framework::db::FieldTypeInfo> {
            vec![#field_types]
        }
    )
}

fn generate_get_fields_as_strings_fn_definition(fields: &venial::NamedFields) 
    -> proc_macro2::TokenStream 
{
    let mut get_fields_as_strings = proc_macro2::TokenStream::new();

    for (field, _) in fields.fields.iter() {
        let field_name = field.name.to_string();
        let field_type = field.ty.clone();

        let mut display_table = None;
        for attribute in &field.attributes {
            if let Some(segment) = attribute.get_single_path_segment() {
                if segment == "display_table" {
                    let value_tokens = attribute.value.get_value_tokens();
                    if value_tokens.len() == 3 {
                        display_table = Some((value_tokens[0].to_string(), 
                            value_tokens[2].to_string()));
                    }
                }
            }
        }

        let display_table_tokens = if let Some(display_table) = display_table {
            let mut table = display_table.0;
            table.pop();
            table.remove(0);
            let mut column = display_table.1;
            column.pop();
            column.remove(0);
            quote::quote!{ display_table: Some((#table.into(), #column.into())) } 
        } else {
            quote::quote!{ display_table: None }
        };

        // TODO: get rid of the unwrap
        let get_field = quote::quote!{
            {
                let table_field = <#field_type>::from_table_field(db_connection, 
                    table_name.clone(), row_id, #field_name.into()).unwrap();
                let display_str_args 
                    = framework::db::TableFieldDisplayStringArgs {
                        db_connection,
                        #display_table_tokens 
                    };
                let display_str = <#field_type>::to_display_string(
                    &table_field, display_str_args);        
                result.push(display_str);
            }
        };

        get_fields_as_strings.extend(get_field);
    }

    quote::quote!(
        fn get_fields_as_strings(db_connection: &DbConnection, 
            table_name: String, row_id: RowId) -> Vec<String> 
        {
            let mut result = Vec::new();
            #get_fields_as_strings
            result
        }
    )
}

// converts a parsed type expression to a type we can use for namespaced 
// function calls e.g. MyTableRowType::setup()
// panics if the type expression can't be expanded
fn type_expr_to_type_str(
    type_expr: &TypeExpr,
) -> String {
    let Some(path) = type_expr.as_path() else {
        panic!("couldn't expand TypeExpr path!");
    };
    let mut result = String::from("");
    for (idx, segment) in
        path.segments.iter().enumerate()
    {
        result += segment.ident.to_string().as_str();
        if idx != path.segments.len() - 1 {
            result += "::";
        }
        if let Some(generic_args) =
            &segment.generic_args
        {
            result += "::";
            result += "<";
            for generic_arg in generic_args.args.iter()
            {
                result += generic_arg
                    .0
                    .to_token_stream()
                    .to_string()
                    .as_str();
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
