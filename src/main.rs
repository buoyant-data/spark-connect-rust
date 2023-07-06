/**
 * The main entrypoint for spark-connect-rust which acts as a simple command line interface
 */
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;

use spark_connect::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    let history = ".spark-connect.history";

    let mut spark = SparkConnect::with("http://[::1]:15002")
        .build()?
        .connect()
        .await?;

    if rl.load_history(&history).is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())
                    .expect("Failed to add history entry");
                let batch = spark.sql(&line).await?;
                let _ = arrow::util::pretty::print_batches(&[batch]);
            }
            Err(ReadlineError::Interrupted) => {
                println!("CTRL-C");
                break;
            }
            Err(ReadlineError::Eof) => {
                println!("CTRL-D");
                break;
            }
            Err(err) => {
                println!("Error: {:?}", err);
                break;
            }
        }
    }
    rl.save_history(history).expect("Failed to save history");
    Ok(())
}
