use winisland_plugin_api::sdk::{Error, Host, Resource};

fn register(host: &Host) -> Result<Resource, Error> {
    host.lyrics()?.register(|line| line.to_uppercase())
}

fn main() {
    let _ = register as fn(&Host) -> Result<Resource, Error>;
}
