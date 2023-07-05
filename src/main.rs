/**
 * The main entrypoint for spark-connect-rust which acts as a simple command line interface
 */
use std::collections::HashMap;

use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use tonic::codegen::*;

use crate::api::spark_connect_service_client::*;

/// The `api` module only contains the gRPC generated stubs from the Spark Connect protos
pub mod api {
    include!("gen/spark.connect.rs");
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut rl = DefaultEditor::new()?;
    let history = ".spark-connect.history";

    // creating a channel ie connection to server
    let channel = tonic::transport::Channel::from_static("http://[::1]:15002")
        .connect()
        .await?;
    // creating gRPC client from channel
    let mut client = SparkConnectServiceClient::new(channel);

    if rl.load_history(&history).is_err() {
        println!("No previous history.");
    }
    loop {
        let readline = rl.readline(">> ");
        match readline {
            Ok(line) => {
                rl.add_history_entry(line.as_str())
                    .expect("Failed to add history entry");
                run_query(&mut client, &line).await?;
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

async fn run_query<T>(
    client: &mut SparkConnectServiceClient<T>,
    query: &str,
) -> Result<(), Box<dyn std::error::Error>>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::Error: Into<StdError>,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    use crate::api::*;

    let plan = Plan {
        op_type: Some(plan::OpType::Command(Command {
            command_type: Some(command::CommandType::SqlCommand(SqlCommand {
                sql: query.into(),
                args: HashMap::default(),
                pos_args: vec![],
            })),
        })),
    };
    let request = ExecutePlanRequest {
        session_id: "lol".into(),
        user_context: None,
        plan: Some(plan),
        client_type: None,
        request_options: vec![],
    };

    let mut stream = client.execute_plan(request).await?.into_inner();
    while let Some(response) = stream.message().await? {
        if let Some(result) = response.response_type {
            match result {
                execute_plan_response::ResponseType::SqlCommandResult(result) => {
                    let relation = result.relation.unwrap();
                    let plan = Plan {
                        op_type: Some(plan::OpType::Root(Relation {
                            common: None,
                            rel_type: Some(relation::RelType::ShowString(Box::new(ShowString {
                                input: Some(Box::new(relation)),
                                num_rows: 10,
                                truncate: 0,
                                vertical: true,
                            }))),
                        })),
                    };
                    let request = ExecutePlanRequest {
                        session_id: "lol".into(),
                        user_context: None,
                        plan: Some(plan),
                        client_type: None,
                        request_options: vec![],
                    };
                    let mut stream = client.execute_plan(request).await?.into_inner();
                    while let Some(response) = stream.message().await? {
                        if let Some(response_type) = response.response_type {
                            match response_type {
                                execute_plan_response::ResponseType::ArrowBatch(batch) => {
                                    let mut reader = arrow_ipc::reader::StreamReader::try_new(
                                        std::io::Cursor::new(batch.data),
                                        None,
                                    )
                                    .expect("Failed to read IPC buffer");
                                    while let Some(result) = reader.next() {
                                        match result {
                                            Ok(batch) => {
                                                let _ =
                                                    arrow::util::pretty::print_batches(&[batch]);
                                            }
                                            Err(err) => {
                                                println!("Something went wrong: {err:?}");
                                            }
                                        };
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }
                unknown => panic!("what's this: {unknown:?}"),
            }
        }
    }
    Ok(())
}
