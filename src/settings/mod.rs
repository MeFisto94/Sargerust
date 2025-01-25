use clap::{Parser, Subcommand, value_parser};
use itertools::Itertools;
use std::str::FromStr;

#[derive(Parser, Debug)]
#[command(name = "Sargerust")]
#[command(version = concat!(env!("VERGEN_GIT_BRANCH"), "/",env!("VERGEN_GIT_SHA")))]
#[command(about = "An open source MMORPG client")]
pub struct CliArgs {
    #[arg(long, env = "SARGERUST_DATA_DIR", default_value_t = default_data_dir())]
    pub data_dir: String,

    #[arg(long, default_value = "enUS", env = "SARGERUST_LOCALE")]
    pub locale: String,

    #[command(subcommand)]
    pub operation_mode: OperationMode,
}

pub fn default_data_dir() -> String {
    std::env::current_dir()
        .expect("Can't read current working directory!")
        .join("_data")
        .to_string_lossy()
        .to_string()
}

#[derive(Subcommand, Debug)]
pub enum OperationMode {
    Standalone {
        map_name: String,
        #[arg(value_parser = value_parser!(Vector3))]
        coordinates: Vector3,
    },
    Remote {
        #[arg(long, default_value = "127.0.0.1")]
        server_host: String,
        #[arg(long, default_value_t = 3724)]
        server_port: u16,
        #[arg(long, env = "SARGERUST_USERNAME")]
        username: String,
        #[arg(
            long,
            env = "SARGERUST_PASSWORD",
            help = "Caution: Both ways of passing this value is insecure. Only use it for dev accounts on local dev servers!"
        )]
        password: String,
    },
}

#[derive(Debug, Clone)]
pub struct Vector3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

fn trim_brackets(input: &str) -> &str {
    let mut chars = input.chars();
    chars.next(); // skip first
    chars.next_back(); // skip last
    chars.as_str()
}

impl FromStr for Vector3 {
    type Err = String;

    // (-a, b, c)
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let string: String = s.chars().filter(|&c| !c.is_whitespace()).collect();
        if !string.starts_with("(") || !string.ends_with(")") {
            return Err("Missing start or end bracket".to_string());
        }

        let trimmed_str = trim_brackets(string.as_str());
        let splits = trimmed_str.split(',').collect_vec();

        if splits.len() != 3 {
            return Err(format!(
                "Comma splitting resulted in {} splits, not 3!",
                splits.len()
            )
            .to_string());
        }

        let components = splits
            .iter()
            // TODO: propagate error better.
            .map(|&split| split.parse::<f32>().expect("Failed to parse component"))
            .collect_vec();

        Ok(Vector3 {
            x: components[0],
            y: components[1],
            z: components[2],
        })
    }
}
