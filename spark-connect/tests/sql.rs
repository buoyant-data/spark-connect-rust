use spark_connect::*;

#[tokio::test]
async fn test_simple_select() -> Result<(), Box<dyn std::error::Error>> {
    let channel = tonic::transport::channel::Channel::from_static("http://[::1]:15002")
        .connect()
        .await?;

    let mut spark = SparkSession::with_client(channel);

    let df = spark.sql("SELECT current_date()").await?;
    assert_eq!(df.session_id, spark.session_id);

    df.show(10, 0).await?;

    Ok(())
}
