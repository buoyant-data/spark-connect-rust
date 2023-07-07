//! The spark-connect-rust library provides an means for Rust applications to integrate
//! with [Spark Connect](https://spark.apache.org/docs/latest/spark-connect-overview.html)
//!
//! ```rust
//! # use spark_connect::*;
//! # #[tokio::main]
//! # async fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let channel = tonic::transport::channel::Channel::from_static("http://[::1]:15002").connect().await?;
//! let mut spark = SparkSession::with_client(channel);
//! let _df = spark.sql("SELECT current_date()").await?;
//! # Ok(())
//! # }
//! ```
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use uuid::Uuid;

/// Generated API stubs based on the Protocol Buffer (`.proto`) definitions for Spark Connect's API
pub mod proto {
    include!("gen/spark.connect.rs");
}

/// Re-exported tonic library
pub use tonic;
use tonic::codegen::{Body, Bytes, StdError};

/// Re-exported errors for API convenience
mod error;
pub use crate::error::Error;
use crate::proto::*;

/// gRPC client for the Spark Connect service
pub use crate::proto::spark_connect_service_client::SparkConnectServiceClient;

/// Type alias for locking mutable [SparkConnectServiceClient] references
type LockedServiceClientRef<T> = Arc<RwLock<SparkConnectServiceClient<T>>>;

/// The primary client interface for interacting with Spark Connect servers
#[derive(Clone, Debug)]
pub struct SparkSession<T> {
    /// Session ID to use for this connection
    pub session_id: Uuid,
    /// Inner gRPC client structure
    inner: LockedServiceClientRef<T>,
}

impl<T> SparkSession<T>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    /// Internal function for initiatilizing a new [SparkConnect]
    fn new(inner: SparkConnectServiceClient<T>) -> Self {
        Self {
            session_id: Uuid::new_v4(),
            inner: Arc::new(RwLock::new(inner)),
        }
    }

    /// Configure [SparkSession] with an already connected transport/client
    ///
    /// Under default circumstances with Tonic this can be done by using a connected
    /// [tonic::transport::channel::Channel] as demonstrated below
    ///
    /// ```rust
    /// # use spark_connect::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let channel = tonic::transport::channel::Channel::from_static("http://[::1]:15002").connect().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn with_client(client: T) -> Self {
        let inner = SparkConnectServiceClient::new(client);
        Self::new(inner)
    }

    /// Read a local file into a [DataFrame] in order to interact with the Spark Connect server
    ///
    /// ```rust
    /// # use std::path::PathBuf;
    /// # use spark_connect::*;
    /// # #[tokio::main]
    /// # async fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # let channel = tonic::transport::channel::Channel::from_static("http://[::1]:15002").connect().await?;
    /// # let mut spark = SparkSession::with_client(channel);
    /// let df = spark.read(PathBuf::from("tests/data/people.csv")).await?;
    /// df.show(10, 0).await?;
    /// # Ok(())
    /// # }
    pub async fn read(&mut self, file: PathBuf) -> Result<DataFrame<T>, Error> {
        Ok(DataFrame::new(self.session_id.clone(), self.inner.clone()))
    }

    /// Send a SQL query to the Spark Connect server
    pub async fn sql(&mut self, query: &str) -> Result<DataFrame<T>, Error> {
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

        if let Ok(mut stream) = self.inner.write() {
            let mut stream = stream.execute_plan(request).await?.into_inner();

            while let Some(response) = stream.message().await? {
                if let Some(result) = response.response_type {
                    match result {
                        execute_plan_response::ResponseType::SqlCommandResult(result) => {
                            let relation = result.relation.unwrap();
                            return Ok(DataFrame {
                                session_id: self.session_id.clone(),
                                inner: self.inner.clone(),
                                relation: Some(relation),
                                data: None,
                            });
                        }
                        unknown => panic!("what's this: {unknown:?}"),
                    }
                }
            }
        }
        Err(Error::Unknown)
    }
}

/// The DataFrame struct looks and feels like a "real" Spark data frame but it isn't actually one
/// because this isn't Spark!
///
/// This is not something that should be directly constructed but instead something returned by
/// other calls on [SparkConnect]
#[derive(Debug)]
pub struct DataFrame<T> {
    pub session_id: Uuid,
    data: Option<bool>,
    relation: Option<Relation>,
    /// Inner gRPC client structure
    inner: LockedServiceClientRef<T>,
}

impl<T> DataFrame<T>
where
    T: tonic::client::GrpcService<tonic::body::BoxBody>,
    T::ResponseBody: Body<Data = Bytes> + Send + 'static,
    <T::ResponseBody as Body>::Error: Into<StdError> + Send,
{
    /// Create a new DataFrame with some defaults.
    ///
    /// This function is not recommended for general use, typically users should be working through
    /// [SparkSession]
    pub fn new(session_id: Uuid, inner: LockedServiceClientRef<T>) -> Self {
        Self {
            session_id,
            inner,
            data: None,
            relation: None,
        }
    }

    /// show_plan is an internal function to make it easier to show() when the [DataFrame] is
    /// created with an existing [Relation], i.e. a Spark Connect query
    async fn show_plan(self, num_rows: i32, truncate: i32) -> Result<Self, Error> {
        let plan = Plan {
            op_type: Some(plan::OpType::Root(Relation {
                common: None,
                rel_type: Some(relation::RelType::ShowString(Box::new(ShowString {
                    input: Some(Box::new(self.relation.clone().unwrap())),
                    num_rows: num_rows,
                    truncate: truncate,
                    vertical: true,
                }))),
            })),
        };

        let request = ExecutePlanRequest {
            session_id: self.session_id.to_string(),
            user_context: None,
            plan: Some(plan),
            client_type: None,
            request_options: vec![],
        };

        if let Ok(mut stream) = self.inner.write() {
            let mut stream = stream.execute_plan(request).await?.into_inner();

            while let Some(response) = stream.message().await? {
                if let Some(response_type) = response.response_type {
                    match response_type {
                        execute_plan_response::ResponseType::ArrowBatch(batch) => {
                            let mut reader = arrow_ipc::reader::StreamReader::try_new(
                                std::io::Cursor::new(batch.data),
                                None,
                            )?;

                            while let Some(result) = reader.next() {
                                let result = result?;
                                #[cfg(feature = "prettyprint")]
                                let _ = arrow::util::pretty::print_batches(&[result]);

                                #[cfg(not(feature = "prettyprint"))]
                                println!("{result:?}");
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(self)
    }

    /// The `show()` function will return a string representation of the given contents of the
    /// [DataFrame]
    pub async fn show(self, num_rows: i32, truncate: i32) -> Result<Self, Error> {
        if self.relation.is_some() {
            return self.show_plan(num_rows, truncate).await;
        }
        // TODO: show when the DataFrame has data
        Ok(self)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_dataframe() {}
}
