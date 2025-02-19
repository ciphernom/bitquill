use gloo_net::http::Request;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use hex;

const DEFAULT_CALENDAR_URL: &str = "https://alice.btc.calendar.opentimestamps.org";

#[derive(Error, Debug)]
pub enum TimestampError {
    #[error("Network error: {0}")]
    Network(String),
    #[error("Calendar error: {0}")]
    Calendar(String),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Timestamp {
    pub digest: String,
    pub timestamp: String,
}

pub struct OpenTimestamps {
    calendar_url: String,
}

impl Default for OpenTimestamps {
    fn default() -> Self {
        Self::new(DEFAULT_CALENDAR_URL.to_string())
    }
}

impl OpenTimestamps {
    pub fn new(calendar_url: String) -> Self {
        Self { calendar_url }
    }

    /// Submits a hash to the OpenTimestamps calendar.
    /// Uses gloo-net's HTTP client (Fetch) to perform the request.
    pub async fn stamp(&self, hash: &str) -> Result<Timestamp, TimestampError> {
        let submit_url = format!("{}/digest", self.calendar_url);

        // Decode the hex string into raw bytes.
        let decoded_hash = hex::decode(hash)
            .map_err(|e| TimestampError::Calendar(e.to_string()))?;

        // Perform the POST request.
        let response = Request::post(&submit_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            // Convert the error from the body method explicitly.
            .body(decoded_hash)
            .map_err(|e| TimestampError::Network(e.to_string()))?
            .send()
            .await
            .map_err(|e| TimestampError::Network(e.to_string()))?;

        if !response.ok() {
            let text = response.text().await
                .map_err(|e| TimestampError::Network(e.to_string()))?;
            return Err(TimestampError::Calendar(format!(
                "Calendar submission failed: {} - {}",
                response.status(),
                text
            )));
        }

        let text = response.text().await
            .map_err(|e| TimestampError::Network(e.to_string()))?;

        Ok(Timestamp {
            digest: hash.to_string(),
            timestamp: text,
        })
    }

    /// Verifies a timestamp with the calendar.
    /// Uses gloo-net's HTTP client (Fetch) to perform the request.
    pub async fn verify(&self, timestamp: &Timestamp) -> Result<bool, TimestampError> {
        let verify_url = format!("{}/verify/{}", self.calendar_url, timestamp.digest);

        let response = Request::get(&verify_url)
            .send()
            .await
            .map_err(|e| TimestampError::Network(e.to_string()))?;

        Ok(response.ok())
    }
}
