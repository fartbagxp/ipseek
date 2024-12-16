use iptrie::Ipv4RTrieSet;
use reqwest::Error;
use serde::Deserialize;
use serde::Serialize;
use std::net::Ipv4Addr;

#[derive(Debug, Deserialize)]
pub struct AwsCidrRanges {
    #[serde(rename = "syncToken")]
    pub sync_token: String,
    #[serde(rename = "createDate")]
    pub create_date: String,
    pub prefixes: Vec<V4CidrBlock>,
    pub ipv6_prefixes: Vec<V6CidrBlock>,
}

#[derive(Debug, Deserialize)]
pub struct V4CidrBlock {
    pub ip_prefix: String,
    pub region: String,
    pub service: String,
    pub network_border_group: String,
}

#[derive(Debug, Deserialize)]
pub struct V6CidrBlock {
    pub ipv6_prefix: String,
    pub region: String,
    pub service: String,
    pub network_border_group: String,
}

#[derive(Debug, Serialize)]
pub struct AwsIpResult {
    pub ip_address: String,
    pub sync_token: String,
    pub create_date: String,
    pub ip_prefix: String,
    pub region: String,
    pub service: String,
    pub network_border_group: String,
}

pub async fn fetch_aws_cidr_ranges() -> Result<AwsCidrRanges, Error> {
    let url =
        "https://raw.githubusercontent.com/fartbagxp/aas-cidr-ranges/master/data/raw/aws.json";
    let response = reqwest::get(url).await?;
    let aws_cidr_ranges: AwsCidrRanges = response.json().await?;
    Ok(aws_cidr_ranges)
}

pub fn find_matching_cidr<'a>(
    trie: &Ipv4RTrieSet,
    ip: Ipv4Addr,
    aws_cidr_ranges: &'a AwsCidrRanges,
) -> Option<&'a V4CidrBlock> {
    let prefix = trie.lookup(&ip);
    aws_cidr_ranges
        .prefixes
        .iter()
        .find(|block| block.ip_prefix == prefix.to_string())
}
