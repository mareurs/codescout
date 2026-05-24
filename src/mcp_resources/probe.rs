//! Debug-only probe resource provider. Registered only when env
//! `CODESCOUT_PROBE=1`. Used to measure whether MCP resource `description`
//! fields and resource bodies are truncated by the client.
//!
//! Two URIs:
//!  - `probe://description-test` — has a sentinel-laden `description` (~9 KB)
//!    in the resource descriptor. Tests the list-time cap.
//!  - `probe://body-test` — short description, but the body returned on read
//!    is ~20 KB with sentinels at known offsets. Tests the read-time cap.

use super::{ResourceBytes, ResourceDescriptor, ResourceError, ResourceProvider};
use std::sync::LazyLock;

pub struct ProbeProvider;

static PROBE_DESC: LazyLock<String> = LazyLock::new(|| build_sentinel_string(9000));
static PROBE_BODY: LazyLock<String> = LazyLock::new(|| build_sentinel_string(20000));

fn build_sentinel_string(target_len: usize) -> String {
    let sentinels: &[(usize, &str)] = &[
        (200, "SENTINEL_0200_AA"),
        (500, "SENTINEL_0500_BB"),
        (1000, "SENTINEL_1000_CC"),
        (1500, "SENTINEL_1500_DD"),
        (2000, "SENTINEL_2000_EE"),
        (2500, "SENTINEL_2500_FF"),
        (3000, "SENTINEL_3000_GG"),
        (4000, "SENTINEL_4000_HH"),
        (5000, "SENTINEL_5000_II"),
        (6000, "SENTINEL_6000_JJ"),
        (8000, "SENTINEL_8000_KK"),
        (10000, "SENTINEL_10000_LL"),
        (12000, "SENTINEL_12000_MM"),
        (15000, "SENTINEL_15000_NN"),
        (18000, "SENTINEL_18000_OO"),
    ];

    let mut s = String::with_capacity(target_len + 200);
    s.push_str(
        "PROBE_BEGIN: diagnostic resource measuring how much of an MCP \
         resource description or body reaches the model. Sentinel markers \
         are embedded at known byte offsets. List every SENTINEL_NNNN_XX \
         token you find verbatim. ",
    );

    for (offset, marker) in sentinels {
        if *offset >= target_len {
            break;
        }
        while s.len() + marker.len() + 4 < *offset {
            s.push_str("filler ");
        }
        s.push_str(marker);
        s.push(' ');
    }

    while s.len() < target_len - 25 {
        s.push_str("filler ");
    }
    s.push_str("SENTINEL_END_C0FFEE");
    s
}

#[async_trait::async_trait]
impl ResourceProvider for ProbeProvider {
    fn descriptors(&self) -> Vec<ResourceDescriptor> {
        vec![
            ResourceDescriptor {
                uri: "probe://description-test".into(),
                name: "probe-description-test".into(),
                description: Some(PROBE_DESC.clone()),
                mime_type: "text/plain".into(),
            },
            ResourceDescriptor {
                uri: "probe://body-test".into(),
                name: "probe-body-test".into(),
                description: Some(
                    "Probe resource — read it to receive a ~20KB body with sentinel \
                     markers at known offsets. Diagnostic only."
                        .into(),
                ),
                mime_type: "text/plain".into(),
            },
        ]
    }

    async fn read(&self, uri: &str) -> Result<ResourceBytes, ResourceError> {
        match uri {
            "probe://description-test" => Ok(ResourceBytes::Text(PROBE_DESC.clone())),
            "probe://body-test" => Ok(ResourceBytes::Text(PROBE_BODY.clone())),
            _ => Err(ResourceError::NotFound(uri.into())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn description_contains_sentinels() {
        let d = PROBE_DESC.as_str();
        for m in [
            "SENTINEL_0200_AA",
            "SENTINEL_2000_EE",
            "SENTINEL_END_C0FFEE",
        ] {
            assert!(d.contains(m), "missing {m}");
        }
    }

    #[test]
    fn body_contains_high_sentinels() {
        let b = PROBE_BODY.as_str();
        for m in [
            "SENTINEL_0200_AA",
            "SENTINEL_10000_LL",
            "SENTINEL_18000_OO",
            "SENTINEL_END_C0FFEE",
        ] {
            assert!(b.contains(m), "missing {m}");
        }
    }
}
