
mod block;
mod blockchain;
mod error;
mod cli;
mod transaction;
mod tx;
mod wallet;
mod utxoset;

use cli::Cli;
use error::Result;
fn main() -> Result<()> {

    let mut cli = Cli::new()?;

    cli.run()?;

    Ok(())
}
