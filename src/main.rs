#![allow(unused)] // silence unused warnings while exploring (to comment out)

use anyhow::{anyhow, bail, Context, Result}; // (xp) (thiserror in prod)
use aws_sdk_s3::{config, ByteStream, Client, Credentials, Region};
use std::env;
use std::fs::{create_dir_all, File};
use std::io::{BufWriter, Write};
use std::path::Path;
use tokio_stream::StreamExt;

// -- constants
const ENV_CRED_KEY_ID: &str = "S3_KEY_ID";
const ENV_CRED_KEY_SECRET: &str = "S3_KEY_SECRET";
const BUCKET_NAME: &str = "rust-aws-sdk-s3-demo";
const REGION: &str = "us-west-2";

#[tokio::main]
async fn main() -> Result<()> {
	let client = get_aws_client(REGION)?;

	let keys = list_keys(&client, BUCKET_NAME).await?;
	println!("List:\n{}", keys.join("\n"));

	let path = Path::new("src/main.rs");
	upload_file(&client, BUCKET_NAME, path).await?;
	println!("Uploaded file {}", path.display());

	let dir = Path::new(".test-data/downloads/");
	let key = "videos/ski-02.mp4";
	download_file(&client, BUCKET_NAME, key, dir).await?;
	println!("Downloaded {key} in directory {}", dir.display());

	Ok(())
}

async fn download_file(client: &Client, bucket_name: &str, key: &str, dir: &Path) -> Result<()> {
	// VALIDATE
	if !dir.is_dir() {
		bail!("Path {} is not a directory", dir.display());
	}

	// create file path and parent dir(s)
	let file_path = dir.join(key);
	let parent_dir = file_path
		.parent()
		.ok_or_else(|| anyhow!("Invalid parent dir for {:?}", file_path))?;
	if !parent_dir.exists() {
		create_dir_all(parent_dir)?;
	}

	// BUILD - aws request
	let req = client.get_object().bucket(bucket_name).key(key);

	// EXECUTE
	let res = req.send().await?;

	// STREAM result to file
	let mut data: ByteStream = res.body;
	let file = File::create(&file_path)?;
	let mut buf_writer = BufWriter::new(file);
	while let Some(bytes) = data.try_next().await? {
		buf_writer.write(&bytes)?;
	}
	buf_writer.flush()?;

	Ok(())
}

async fn upload_file(client: &Client, bucket_name: &str, path: &Path) -> Result<()> {
	// VALIDATE
	if !path.exists() {
		bail!("Path {} does not exists", path.display());
	}
	let key = path.to_str().ok_or_else(|| anyhow!("Invalid path {path:?}"))?;

	// PREPARE
	let body = ByteStream::from_path(&path).await?;
	let content_type = mime_guess::from_path(&path).first_or_octet_stream().to_string();

	// BUILD - aws request
	let req = client
		.put_object()
		.bucket(bucket_name)
		.key(key)
		.body(body)
		.content_type(content_type);

	// EXECUTE
	req.send().await?;

	Ok(())
}

async fn list_keys(client: &Client, bucket_name: &str) -> Result<Vec<String>> {
	// BUILD - aws request
	let req = client.list_objects_v2().prefix("").bucket(bucket_name);

	// EXECUTE
	let res = req.send().await?;

	// COLLECT
	let keys = res.contents().unwrap_or_default();
	let keys = keys
		.iter()
		.filter_map(|o| o.key.as_ref())
		.map(|s| s.to_string())
		.collect::<Vec<_>>();

	Ok(keys)
}

fn get_aws_client(region: &str) -> Result<Client> {
	// get the id/secret from env
	let key_id = env::var(ENV_CRED_KEY_ID).context("Missing S3_KEY_ID")?;
	let key_secret = env::var(ENV_CRED_KEY_SECRET).context("Missing S3_KEY_SECRET")?;

	// build the aws cred
	let cred = Credentials::new(key_id, key_secret, None, None, "loaded-from-custom-env");

	// build the aws client
	let region = Region::new(region.to_string());
	let conf_builder = config::Builder::new().region(region).credentials_provider(cred);
	let conf = conf_builder.build();

	// build aws client
	let client = Client::from_conf(conf);
	Ok(client)
}
