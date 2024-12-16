pub mod aws;
pub mod utils;
pub mod handler;

use aws::{fetch_aws_cidr_ranges, find_matching_cidr, V4CidrBlock};
use handler::check_ip;
use utils::{AppState, build_tries};
