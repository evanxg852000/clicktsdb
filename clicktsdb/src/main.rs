mod settings;
mod web;

use structopt::StructOpt;

use anyhow::{Ok, Result};

use crate::{
    settings::{CommandLineArgs, Settings},
    web::serve,
};

#[tokio::main]
async fn main() -> Result<()> {
    let command_line_args = CommandLineArgs::from_args();
    // println!("Welcome to ClickTSDB");

    // initialize tracing
    // tracing_subscriber::fmt::init();

    // load settings
    let settings = Settings::load(command_line_args.config)?;

    // start web service
    serve(settings).await?;

    Ok(())
}
