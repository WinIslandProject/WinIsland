use winisland_plugin_api::sdk::{Error, Host, Resource};

fn register(host: &Host) -> Result<Resource, Error> {
    host.media()?.create_source("Sample track", "Sample artist")
}

fn main() {
    let _ = register as fn(&Host) -> Result<Resource, Error>;
}
