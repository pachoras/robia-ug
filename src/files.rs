// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::env::{self, VarError};

use aws_sdk_s3::primitives::ByteStream;
use axum::body::Bytes;

#[derive(Debug)]
pub struct S3Error(String);
impl S3Error {
    pub fn new(value: impl Into<String>) -> Self {
        S3Error(value.into())
    }

    pub fn add_message(self, message: impl Into<String>) -> Self {
        S3Error(format!("{}: {}", message.into(), self.0))
    }
}

impl<T: aws_sdk_s3::error::ProvideErrorMetadata> From<T> for S3Error {
    fn from(value: T) -> Self {
        S3Error(format!(
            "{}: {}",
            value
                .code()
                .map(String::from)
                .unwrap_or("unknown code".into()),
            value
                .message()
                .map(String::from)
                .unwrap_or("missing reason".into()),
        ))
    }
}

impl std::error::Error for S3Error {}

impl std::fmt::Display for S3Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Copy an object from one bucket to another.
pub async fn copy_object(
    client: &aws_sdk_s3::Client,
    source_bucket: &str,
    destination_bucket: &str,
    source_object: &str,
    destination_object: &str,
) -> Result<(), S3Error> {
    let source_key = format!("{source_bucket}/{source_object}");
    let response = client
        .copy_object()
        .copy_source(&source_key)
        .bucket(destination_bucket)
        .key(destination_object)
        .send()
        .await?;

    println!(
        "Copied from {source_key} to {destination_bucket}/{destination_object} with etag {}",
        response
            .copy_object_result
            .unwrap_or_else(|| aws_sdk_s3::types::CopyObjectResult::builder().build())
            .e_tag()
            .unwrap_or("missing")
    );
    Ok(())
}

/// Delete an object from a bucket.
pub async fn remove_object(
    client: &aws_sdk_s3::Client,
    bucket: &str,
    key: &str,
) -> Result<(), S3Error> {
    client
        .delete_object()
        .bucket(bucket)
        .key(key)
        .send()
        .await?;

    // There are no modeled errors to handle when deleting an object.
    Ok(())
}

pub async fn download_object(
    client: &aws_sdk_s3::Client,
    bucket_name: &str,
    key: &str,
) -> Result<aws_sdk_s3::operation::get_object::GetObjectOutput, S3Error> {
    client
        .get_object()
        .bucket(bucket_name)
        .key(key)
        .send()
        .await
        .map_err(S3Error::from)
}

pub async fn upload_object(
    client: &aws_sdk_s3::Client,
    bucket_name: &str,
    key: &str,
    body: ByteStream,
) -> Result<aws_sdk_s3::operation::put_object::PutObjectOutput, S3Error> {
    client
        .put_object()
        .bucket(bucket_name)
        .key(key)
        .body(body)
        .send()
        .await
        .map_err(S3Error::from)
}

pub async fn list_objects(client: &aws_sdk_s3::Client, bucket: &str) -> Result<(), S3Error> {
    let mut response = client
        .list_objects_v2()
        .bucket(bucket.to_owned())
        .max_keys(10) // In this example, go 10 at a time.
        .into_paginator()
        .send();

    while let Some(result) = response.next().await {
        match result {
            Ok(output) => {
                for object in output.contents() {
                    println!(" - {}", object.key().unwrap_or("Unknown"));
                }
            }
            Err(err) => {
                eprintln!("{err:?}")
            }
        }
    }

    Ok(())
}

/// Given a bucket, remove all objects in the bucket, and then ensure no objects
/// remain in the bucket.
pub async fn clear_bucket(
    client: &aws_sdk_s3::Client,
    bucket_name: &str,
) -> Result<Vec<String>, S3Error> {
    let objects = client.list_objects_v2().bucket(bucket_name).send().await?;

    // delete_objects no longer needs to be mutable.
    let objects_to_delete: Vec<String> = objects
        .contents()
        .iter()
        .filter_map(|obj| obj.key())
        .map(String::from)
        .collect();

    if objects_to_delete.is_empty() {
        return Ok(vec![]);
    }

    let return_keys = objects_to_delete.clone();

    delete_objects(client, bucket_name, objects_to_delete).await?;

    let objects = client.list_objects_v2().bucket(bucket_name).send().await?;

    eprintln!("{objects:?}");

    match objects.key_count {
        Some(0) => Ok(return_keys),
        _ => Err(S3Error::new("There were still objects left in the bucket.")),
    }
}

pub async fn delete_objects(
    client: &aws_sdk_s3::Client,
    bucket_name: &str,
    objects_to_delete: Vec<String>,
) -> Result<(), S3Error> {
    // Push into a mut vector to use `?` early return errors while building object keys.
    let mut delete_object_ids: Vec<aws_sdk_s3::types::ObjectIdentifier> = vec![];
    for obj in objects_to_delete {
        let obj_id = aws_sdk_s3::types::ObjectIdentifier::builder()
            .key(obj)
            .build()
            .map_err(|err| {
                S3Error::new(format!("Failed to build key for delete_object: {err:?}"))
            })?;
        delete_object_ids.push(obj_id);
    }

    client
        .delete_objects()
        .bucket(bucket_name)
        .delete(
            aws_sdk_s3::types::Delete::builder()
                .set_objects(Some(delete_object_ids))
                .build()
                .map_err(|err| {
                    S3Error::new(format!("Failed to build delete_object input {err:?}"))
                })?,
        )
        .send()
        .await?;
    Ok(())
}

pub async fn create_bucket(
    client: &aws_sdk_s3::Client,
    bucket_name: &str,
    region: &aws_config::Region,
) -> Result<Option<aws_sdk_s3::operation::create_bucket::CreateBucketOutput>, S3Error> {
    let constraint = aws_sdk_s3::types::BucketLocationConstraint::from(region.to_string().as_str());
    let cfg = aws_sdk_s3::types::CreateBucketConfiguration::builder()
        .location_constraint(constraint)
        .build();
    let create = client
        .create_bucket()
        .create_bucket_configuration(cfg)
        .bucket(bucket_name)
        .send()
        .await;

    // BucketAlreadyExists and BucketAlreadyOwnedByYou are not problems for this task.
    create.map(Some).or_else(|err| {
        if err
            .as_service_error()
            .map(|se| se.is_bucket_already_exists() || se.is_bucket_already_owned_by_you())
            == Some(true)
        {
            Ok(None)
        } else {
            Err(S3Error::from(err))
        }
    })
}

pub async fn delete_bucket(client: &aws_sdk_s3::Client, bucket_name: &str) -> Result<(), S3Error> {
    let resp = client.delete_bucket().bucket(bucket_name).send().await;
    match resp {
        Ok(_) => Ok(()),
        Err(err) => {
            if err
                .as_service_error()
                .and_then(aws_sdk_s3::error::ProvideErrorMetadata::code)
                == Some("NoSuchBucket")
            {
                Ok(())
            } else {
                Err(S3Error::from(err))
            }
        }
    }
}

pub struct Config {
    pub region: String,
    pub access_key: String,
    pub secret_key: String,
    pub endpoint_url: String,
    pub bucket: String,
}

impl Config {
    pub fn from_env() -> Result<Self, VarError> {
        let region = env::var("RUSTFS_REGION")?;
        let access_key = env::var("RUSTFS_ACCESS_KEY")?;
        let secret_key = env::var("RUSTFS_SECRET_KEY")?;
        let endpoint_url = env::var("RUSTFS_ENDPOINT_URL")?;
        let bucket = env::var("RUSTFS_BUCKET")?;

        Ok(Config {
            region,
            access_key,
            secret_key,
            endpoint_url,
            bucket,
        })
    }
}

pub async fn initialize_s3_client() -> aws_sdk_s3::Client {
    let config = Config::from_env().expect(
        r#"Failed to load S3 configuration from environment variables. 
            Please ensure RUSTFS_REGION, RUSTFS_ACCESS_KEY, RUSTFS_SECRET_KEY, 
            and RUSTFS_ENDPOINT_URL are set."#,
    );

    let credentials = aws_sdk_s3::config::Credentials::new(
        config.access_key,
        config.secret_key,
        None,
        None,
        "rustfs",
    );
    let region = aws_config::Region::new(config.region);
    let endpoint_url = config.endpoint_url;
    let shard_config = aws_config::defaults(aws_config::BehaviorVersion::latest())
        .region(region)
        .credentials_provider(credentials)
        .endpoint_url(endpoint_url)
        .load()
        .await;

    // Create bucket if it doesn't exist
    let client = aws_sdk_s3::Client::new(&shard_config);
    create_bucket(
        &client,
        &config.bucket,
        &aws_config::Region::new("us-east-1"),
    )
    .await
    .expect("Failed to create bucket");
    client
}

pub async fn upload_file_to_s3(
    client: &aws_sdk_s3::Client,
    file_name: &str,
    data: Bytes,
) -> Result<(), S3Error> {
    let config = Config::from_env().unwrap();
    let byte_stream = ByteStream::from(data);
    // Upload the file
    upload_object(client, &config.bucket, file_name, byte_stream).await?;
    Ok(())
}
