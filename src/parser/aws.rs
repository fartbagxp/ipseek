use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CidrBlock {
    ip_prefix: String,
    region: String,
    service: String,
    network_border_group: String,
}

#[derive(Debug, Deserialize)]
pub struct AwsCidrRanges {
    sync_token: String,
    create_date: String,
    prefixes: Vec<CidrBlock>,
}