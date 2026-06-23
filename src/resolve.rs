//! Shared resolution logic: turn a set of matching IP-range entries into a
//! layered answer that distinguishes a *service* running on top of the
//! *cloud provider / infrastructure* it is hosted on.
//!
//! A single IP can match several published ranges at once (e.g. a Stripe or
//! Datadog range that physically lives inside an AWS range). Rather than
//! collapsing that to one answer, we keep every distinct provider and order
//! them service-first, cloud-last so the layering is obvious.

use crate::db::StoredEntry;
use serde::Serialize;
use std::collections::HashMap;

/// Normalise a raw provider/file name (e.g. `azure-public.json`,
/// `grafana-hosted-alerts.txt`) into a short canonical token.
pub fn canonical_provider(name: &str) -> String {
    let mut s = name.to_lowercase();
    if s.ends_with(".json") {
        s = s.trim_end_matches(".json").to_string();
    }
    if s.ends_with(".txt") {
        s = s.trim_end_matches(".txt").to_string();
    }
    let mut tokens: Vec<&str> = s
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    let stopwords = [
        "ips",
        "ipv4",
        "ipv6",
        "v4",
        "v6",
        "ip",
        "iprange",
        "ip-range",
        "range",
        "ranges",
        "public",
        "hosted",
        "cdn",
        "api",
        "webhook",
        "prefixes",
        "prefix",
        "sip",
        "synthetics",
        "atlas",
        "relay",
        "private",
    ];
    while tokens.len() > 1 {
        let last = *tokens.last().unwrap();
        if stopwords.contains(&last) || last.starts_with("ipv") || last.starts_with("ips") {
            tokens.pop();
        } else {
            break;
        }
    }
    let joined = tokens.join("-");
    collapse_family(&joined)
}

/// Collapse the many per-cloud files of a single vendor into one provider name
/// (e.g. zscaler/zdx publish ~20 overlapping hub files). Keeps output legible
/// without losing the vendor.
fn collapse_family(name: &str) -> String {
    if name.contains("zscaler") || name.contains("zscloud") || name.starts_with("zdx") {
        return "zscaler".to_string();
    }
    // asn-PROVIDER-asNNNN → PROVIDER  (e.g. asn-alibaba-as37963 → alibaba)
    if let Some(rest) = name.strip_prefix("asn-") {
        let provider: String = rest
            .split('-')
            .filter(|t| !(t.starts_with("as") && t[2..].chars().all(|c| c.is_ascii_digit())))
            .collect::<Vec<_>>()
            .join("-");
        return if provider.is_empty() {
            rest.to_string()
        } else {
            provider
        };
    }
    // PROVIDER-asNNNN → PROVIDER  (e.g. hetzner-as24940 → hetzner)
    if let Some(pos) = name.rfind('-') {
        let tail = &name[pos + 1..];
        if tail.starts_with("as") && tail[2..].chars().all(|c| c.is_ascii_digit()) {
            return name[..pos].to_string();
        }
    }
    // m365-china / m365-usgov-* / m365-worldwide → m365
    if name.starts_with("m365") {
        return "m365".to_string();
    }
    name.to_string()
}

/// Whether a provider is underlying cloud infrastructure (IaaS) or a service
/// that typically runs on top of such infrastructure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Cloud,
    Service,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Cloud => "cloud",
            Kind::Service => "service",
        }
    }
}

/// Provider tokens that represent cloud / hosting infrastructure (IaaS).
/// Anything not listed here is treated as a service overlay.
const CLOUD_PROVIDERS: &[&str] = &[
    "aws",
    "amazon",
    "azure",
    "gcp",
    "google",
    "oracle",
    "oci",
    "digitalocean",
    "linode",
    "vultr",
    "hetzner",
    "ovh",
    "ibm",
    "alibaba",
    "tencent",
    "scaleway",
    "baidu",
    "huawei",
    "ionos",
];

/// Classify a *canonical* provider token as cloud infrastructure or a service.
pub fn classify(canonical: &str) -> Kind {
    let head = canonical.split('-').next().unwrap_or(canonical);
    if CLOUD_PROVIDERS.contains(&head) {
        Kind::Cloud
    } else {
        Kind::Service
    }
}

/// One layer of the answer: a single provider that claims the IP.
#[derive(Debug, Clone, Serialize)]
pub struct Layer {
    pub provider: String,
    pub kind: Kind,
    pub prefix: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub service: Option<String>,
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, String>,
}

/// The full resolved answer for an IP.
#[derive(Debug, Clone, Serialize)]
pub struct Resolution {
    /// Layers ordered service-first (overlay) to cloud-last (infrastructure).
    pub layers: Vec<Layer>,
    /// One-line human readable summary, e.g. `stripe (service) on aws (cloud · us-east-1)`.
    pub summary: String,
}

impl Resolution {
    /// The most-specific / overlay layer, used for backward-compatible
    /// top-level `provider`/`region`/`service` fields.
    pub fn primary(&self) -> &Layer {
        &self.layers[0]
    }
}

fn norm(v: Option<String>) -> Option<String> {
    match v {
        Some(s) if !s.is_empty() => Some(s),
        _ => None,
    }
}

/// Build a [`Resolution`] from every matching entry and its prefix length.
/// Returns `None` when there are no matches.
pub fn resolve(matches: Vec<(u8, StoredEntry)>) -> Option<Resolution> {
    if matches.is_empty() {
        return None;
    }

    // Collapse to one layer per provider, keeping the most specific prefix seen
    // for that provider.
    let mut by_provider: Vec<(u8, Layer)> = Vec::new();
    for (len, e) in matches {
        let provider = canonical_provider(&e.provider);
        let kind = classify(&provider);
        if let Some(slot) = by_provider.iter_mut().find(|(_, l)| l.provider == provider) {
            if len > slot.0 {
                slot.0 = len;
                slot.1.prefix = e.prefix.clone();
                slot.1.region = norm(e.region.clone());
                slot.1.service = norm(e.service.clone());
                slot.1.metadata = e.metadata.fields.clone();
            }
            continue;
        }
        by_provider.push((
            len,
            Layer {
                provider,
                kind,
                prefix: e.prefix,
                region: norm(e.region),
                service: norm(e.service),
                metadata: e.metadata.fields,
            },
        ));
    }

    // Order: services (overlay) before clouds (infrastructure); within each
    // group, most-specific prefix first. Stable so equal keys keep order.
    by_provider.sort_by(|a, b| {
        let ka = matches!(a.1.kind, Kind::Cloud) as u8;
        let kb = matches!(b.1.kind, Kind::Cloud) as u8;
        ka.cmp(&kb).then(b.0.cmp(&a.0))
    });

    let layers: Vec<Layer> = by_provider.into_iter().map(|(_, l)| l).collect();
    let summary = build_summary(&layers);
    Some(Resolution { layers, summary })
}

fn layer_desc(l: &Layer) -> String {
    let mut extra: Vec<String> = Vec::new();
    extra.push(l.kind.as_str().to_string());
    if let Some(r) = &l.region {
        extra.push(r.clone());
    }
    if let Some(s) = &l.service {
        extra.push(s.clone());
    }
    format!("{} ({})", l.provider, extra.join(" · "))
}

fn build_summary(layers: &[Layer]) -> String {
    layers
        .iter()
        .map(layer_desc)
        .collect::<Vec<_>>()
        .join(" on ")
}
