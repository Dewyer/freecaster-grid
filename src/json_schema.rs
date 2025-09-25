use anyhow::Context;

use crate::config::Config;

pub fn generate_json_schema() -> anyhow::Result<()> {
    let schema = schemars::schema_for!(Config);
    let schema = serde_json::to_string(&schema)?;

    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 2 {
        eprintln!("Usage: {} <output_path>", args[0]);
        std::process::exit(1);
    }

    let output_path = &args[1];
    std::fs::write(output_path, schema).context("Failed to write JSON schema to file")?;
    std::process::exit(0);
}
