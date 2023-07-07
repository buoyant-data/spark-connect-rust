//! The spark-connect-rust library provides an means for Rust applications to integrate
//! with [Spark Connect](https://spark.apache.org/docs/latest/spark-connect-overview.html)
//!
//! ```rust
//! # use spark_connect::*;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let channel = tonic::transport::channel::Channel::from_static("http://[::1]:15002").connect().await?;
//! let spark = SparkConnect::with_client(channel);
//! # Ok(())
//! # }
//! ```
use std::collections::HashMap;

use uuid::Uuid;

/// Generated API stubs based on the Protocol Buffer (`.proto`) definitions for Spark Connect's API
pub mod proto {
    include!("gen/spark.connect.rs");
}

/// Re-exported tonic library
pub use tonic;

/// Re-exported errors for API convenience
mod error;
pub use crate::error::Error;

/// gRPC client for the Spark Connect service
pub use crate::proto::spark_connect_service_client::SparkConnectServiceClient;

/// The primary client interface for interacting with SparkConnect instances
#[derive(Clone, Debug)]
pub struct SparkConnect<T> {
    /// Session ID to use for this connection
    session_id: Uuid,
    /// Inner gRPC client structure
    inner: SparkConnectServiceClient<T>,
}

use tonic::codegen::{Body, Bytes, StdError};

impl<T> SparkConnect<T>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    fn new(inner: SparkConnectServiceClient<T>) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            inner,
        }
    }

    pub fn with_client(client: T) -> Self {
        let inner = SparkConnectServiceClient::new(client);
        Self::new(inner)
    }

    /// Send a SQL query to the Spark Connect server
    pub async fn sql(&mut self, query: &str) -> Result<arrow_array::RecordBatch, Error> {
        use crate::proto::*;
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
            session_id: self.session_id.to_string(),
            user_context: None,
            plan: Some(plan),
            client_type: None,
            request_options: vec![],
        };

        let mut stream = self.inner.execute_plan(request).await?.into_inner();

        while let Some(response) = stream.message().await? {
            if let Some(result) = response.response_type {
                match result {
                    execute_plan_response::ResponseType::SqlCommandResult(result) => {
                        let relation = result.relation.unwrap();
                        let plan = Plan {
                            op_type: Some(plan::OpType::Root(Relation {
                                common: None,
                                rel_type: Some(relation::RelType::ShowString(Box::new(
                                    ShowString {
                                        input: Some(Box::new(relation)),
                                        num_rows: 10,
                                        truncate: 0,
                                        vertical: true,
                                    },
                                ))),
                            })),
                        };
                        let request = ExecutePlanRequest {
                            session_id: "lol".into(),
                            user_context: None,
                            plan: Some(plan),
                            client_type: None,
                            request_options: vec![],
                        };

                        let mut stream = self.inner.execute_plan(request).await?.into_inner();
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
                                            return result.map_err(Error::ArrowError);
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
        Err(Error::Unknown)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /*
    #[tokio::test]
    async fn test_build_sparkconnect() -> Result<(), Error>  {
        let host = "http://example.com";
        let spark = SparkConnect::with(host).await?;
        assert_eq!(spark.host, host);
        Ok(())
    }
    */
}
