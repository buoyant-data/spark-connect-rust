//! The spark-connect-rust library provides an means for Rust applications to integrate
//! with [Spark Connect](https://spark.apache.org/docs/latest/spark-connect-overview.html)
//!
//! ```rust
//! # use spark_connect::*;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let spark = SparkConnect::with("http://[::1]:15002")
//!                 .build()?
//!                 .connect()
//!                 .await?;
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
#[derive(Clone, Debug, Default)]
pub struct SparkConnect {
    /// Configured hostname for the Spark Connect connection
    host: String,
    /// Session ID to use for this connection
    session_id: Uuid,
    /// Inner gRPC client structure
    inner: Option<SparkConnectServiceClient<tonic::transport::channel::Channel>>,
}

impl SparkConnect {
    /// Create a [SparkConnectBuilder] with the given hostname to a Spark Connect server
    pub fn with(host: &str) -> SparkConnectBuilder {
        let mut builder = SparkConnectBuilder::default();
        builder.host = Some(host.into());
        builder
    }

    /// Connect the configured [SparkConnect] instance to the configured `host`
    pub async fn connect(mut self) -> Result<Self, Error> {
        let uri: tonic::transport::Uri = self.host.parse()?;
        // creating a channel ie connection to server
        let channel = tonic::transport::Channel::builder(uri).connect().await?;
        self.inner = Some(SparkConnectServiceClient::new(channel));

        Ok(self)
    }

    /// Send a SQL query to the Spark Connect server
    pub async fn sql(&mut self, query: &str) -> Result<arrow::record_batch::RecordBatch, Error> {
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

        let mut stream = self
            .inner
            .as_mut()
            .unwrap()
            .execute_plan(request)
            .await?
            .into_inner();

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

                        let mut stream = self
                            .inner
                            .as_mut()
                            .unwrap()
                            .execute_plan(request)
                            .await?
                            .into_inner();
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

/// Internal builder object for creating new [SparkConnect] objects
#[derive(Clone, Debug, Default)]
pub struct SparkConnectBuilder {
    host: Option<String>,
}

impl SparkConnectBuilder {
    /// Build the configured [SparkConnect] instance
    pub fn build(self) -> Result<SparkConnect, Error> {
        match self.host {
            Some(host) => Ok(SparkConnect {
                session_id: Uuid::new_v4(),
                inner: None,
                host,
            }),
            None => Err(Error::Unknown),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_sparkconnect() {
        let host = "http://example.com";
        let spark = SparkConnect::with(host).build().expect("Failed to build");
        assert_eq!(spark.host, host);
    }
}
