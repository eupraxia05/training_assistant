use framework::prelude::*;

#[test]
fn db_commands_test() -> Result<()> {
    let mut context = Context::new();
    context.add_plugin(DbPlugin);
    context.add_plugin(training::TrainingPlugin);
    context.in_memory_db(true);

    context.startup()?;

    let info_response = context.execute("db info")?;
    assert!(info_response.text().is_some());
    assert!(info_response.text().unwrap() == "Database connection open.\nNo database path (in-memory connection)");

    assert_eq!(context.db_connection()?.get_table_row_ids("trainer")?, vec![]);
    let new_response = context.execute("new --table=trainer")?;
    assert!(new_response.text().is_some());
    assert_eq!(new_response.text().unwrap(), "Inserted new row (id: 1) in table trainer.");
    assert_eq!(context.db_connection()?.get_table_row_ids("trainer")?, vec![1]); 

    let list_response = context.execute("list --table=trainer")?;
    println!("{}", list_response.text().unwrap());
    assert_eq!(list_response.text().unwrap(), 
        "+----+------+--------------+---------+-------+-------+\n\
        | ID | name | company_name | address | email | phone |\n\
        +----+------+--------------+---------+-------+-------+\n\
        | 1  | Err  |              |         |       |       |\n\
        +----+------+--------------+---------+-------+-------+"
    );
    context.execute("db erase")?;
    assert!(!context.db_connection()?.is_open());

    Ok(())
}
