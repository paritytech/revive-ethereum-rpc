use std::path::Path;

use crate::generator::TypeGenerator;

#[macro_use]
extern crate lazy_static;
mod generator;
mod open_rpc;
mod printer;

fn main() -> anyhow::Result<()> {
    let specs = generator::read_specs()?;

    let mut generator = TypeGenerator::new();
    generator.collect_extra_type("TransactionUnsigned");

    let out_dir = if let Ok(dir) = std::env::var("CARGO_MANIFEST_DIR") {
        Path::new(&dir).join("../api/src")
    } else {
        "./rpc/api/src".into()
    };
    let out = std::fs::canonicalize(out_dir.join("rpc_methods.rs"))?;
    println!("Generating rpc_methods at {out:?}");
    std::fs::write(out, generator.generate_rpc_methods(&specs)).expect("Unable to write file");

    let out = std::fs::canonicalize(out_dir.join("rpc_types.rs"))?;
    println!("Generating rpc_types at {out:?}");
    std::fs::write(out, generator.generate_types(&specs)).expect("Unable to write file");
    Ok(())
}
