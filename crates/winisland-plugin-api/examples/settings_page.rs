use winisland_plugin_api::sdk::{Error, Host, Resource};

fn register(host: &Host) -> Result<Resource, Error> {
    host.settings()?
        .create_label_page("sample", "Sample settings", "Plugin is ready")
}

fn main() {
    let _ = register as fn(&Host) -> Result<Resource, Error>;
}
