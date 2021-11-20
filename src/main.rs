#[macro_use]
extern crate derive_new;

use futures::future;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::{Client, StatusCode};
use std::io::Write;
use structopt::StructOpt;
use tokio;
use tracing::{self, Level};
use tracing_subscriber::FmtSubscriber;

#[derive(Debug, new)]
struct Resource {
    url: String,
    fp: std::path::PathBuf,
}

async fn dl_url(client: &Client, resource: &Resource) {
    let has_file = std::path::Path::new(&resource.fp).exists();
    if has_file {
        tracing::info!("skipping {:?}", resource);
        return;
    }

    match client.get(&resource.url).send().await {
        Err(err) => tracing::error!("request error {:?}", err),
        Ok(resp) => {
            if resp.status() != StatusCode::OK {
                tracing::error!("none 200 resp on {:?}", resource)
            }

            match resp.bytes().await {
                Err(parse_bytes_err) => {
                    tracing::error!("unable to parse bytes {}", parse_bytes_err)
                }
                Ok(bytes) => match std::fs::File::create(&resource.fp) {
                    Err(create_file_error) => {
                        tracing::error!("unable to create file {}", create_file_error)
                    }
                    Ok(mut file) => match file.write(&bytes) {
                        Err(write_file_error) => {
                            tracing::error!("unable to write file {}", write_file_error)
                        }
                        Ok(_) => {}
                    },
                },
            }
        }
    }
}

async fn dl_url_pb(client: &Client, resource: &Resource, pb: &ProgressBar) {
    dl_url(client, resource).await;
    pb.inc(1);
}

async fn batch_dl_pb(client: &Client, resources: &Vec<Resource>, pb: &ProgressBar) {
    let futures = resources.iter().map(|res| dl_url_pb(client, res, pb));
    let _ = future::join_all(futures).await;
}

fn mk_fp(url: &str, dirpath: std::path::PathBuf) -> Option<std::path::PathBuf> {
    match url.split('/').last() {
        None => {
            tracing::error!("unable to make a file name for {}", url);
            None
        }
        Some(filename) => {
            let mut out = dirpath;
            out.push(filename);
            Some(out)
        }
    }
}

fn read_urls(fp: std::path::PathBuf) -> anyhow::Result<Vec<String>> {
    let content = std::fs::read_to_string(fp)?;
    let urls = content
        .split('\n')
        .map(|url| url.trim().to_string())
        .collect::<Vec<_>>();

    Ok(urls)
}

#[derive(Debug, StructOpt)]
#[structopt(name = "dl-res", about = "download resources from given url files")]
struct Opt {
    /// number of concurrent fetching
    #[structopt(short = "n", default_value = "10")]
    batch_size: usize,
    /// the path to the batch files
    #[structopt(short = "i", default_value = "urls.txt")]
    batch_file_path: String,
    /// the output folder path
    #[structopt(short = "o", default_value = "out")]
    out_path: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // a builder for `FmtSubscriber`.
    let subscriber = FmtSubscriber::builder()
        // all spans/events with a level higher than TRACE (e.g, debug, info, warn, etc.)
        // will be written to stdout.
        .with_max_level(Level::INFO)
        // completes the builder.
        .finish();

    tracing::subscriber::set_global_default(subscriber).expect("setting default subscriber failed");

    let opt = Opt::from_args();

    let mut exe_dir_path = std::env::current_exe()?;
    exe_dir_path.push("..");
    let exe_dir_path = exe_dir_path;

    // check has out dir
    let mut out_path = exe_dir_path.clone();
    out_path.push(&opt.out_path);
    if !out_path.exists() || !out_path.is_dir() {
        anyhow::bail!("{} is not a valid folder", out_path.to_str().unwrap());
    }

    let mut in_path = exe_dir_path.clone();
    in_path.push(&opt.batch_file_path);
    let urls = read_urls(in_path)?;

    let client = reqwest::Client::new();

    let pb = ProgressBar::new(urls.len() as u64);
    pb.set_style(
        ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{wide_bar:.cyan/blue}] {pos}/{len} {eta}",
            )
            .progress_chars("#>-"),
    );

    for batch_url in urls.chunks(opt.batch_size) {
        let resources = batch_url
            .iter()
            .map(|url| Resource::new(url.clone(), mk_fp(url, out_path.clone()).unwrap()))
            .collect::<Vec<_>>();

        batch_dl_pb(&client, &resources, &pb).await;
    }

    pb.finish();

    Ok(())
}
