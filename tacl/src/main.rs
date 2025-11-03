use framework::prelude::*;
use billing::InvoicePlugin;

fn main() {
    let mut context = Context::new();
    context.add_plugin(DbPlugin::default())
        .add_plugin(InvoicePlugin::default());

    let mut command_args = std::env::args().collect::<Vec<_>>();

    // remove the initial executable name from args
    // todo: this isn't guaranteed to be the executable name, should probably check it's what we expect
    command_args.remove(0);
    
    context.execute(shlex::try_join(command_args.iter().map(|e| e.as_str())).expect("failed to join args").as_str()).expect("command failed");
}

