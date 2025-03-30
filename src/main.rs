use auto_args::AutoArgs;
use ir::send_settings;

mod ir;

#[derive(Debug, AutoArgs)]
pub struct Settings {
    /// On or off
    pub power: Power,
    /// Mode like heating or cooling
    pub mode: Mode,
    /// Temperature in celsius (16-31)
    pub temp: u8,
    /// Fan speed
    pub fan: Fan,
}

#[derive(Debug, AutoArgs)]
pub enum Power {
    On,
    Off,
}

#[derive(Debug, AutoArgs)]
pub enum Mode {
    Heat,
    Dry,
    Cool,
    Fan,
}

#[derive(Debug, AutoArgs)]
pub enum Fan {
    Auto,
    Low,
    Medium,
    High,
    Higher,
}

fn main() {
    let settings = Settings::from_args();

    // Only supported on Linux
    if cfg!(not(target_os = "linux")) {
        println!("Not supported on this platform");
        std::process::exit(1);
    }

    send_settings(settings).unwrap();
}
