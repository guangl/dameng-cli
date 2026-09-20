use dm_plugin_sdk::{Context, Plugin, PluginResult};

struct Hello;
impl Plugin for Hello {
    fn run(&self, context: Context) -> PluginResult {
        if context.args.first().is_some_and(|arg| arg == "--help") {
            println!("Usage: dm hello [arguments...]\nRust plugin protocol example.");
            return Ok(0);
        }
        println!("Hello from a Rust dm plugin!");
        for arg in context.args {
            println!("{}", arg.to_string_lossy());
        }
        Ok(0)
    }
}

fn main() {
    dm_plugin_sdk::run(Hello);
}
