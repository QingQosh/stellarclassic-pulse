// Example client for consuming streaming JSON responses from SorobanPulse
// This demonstrates best practices for handling large result sets

use reqwest::Client;
use serde_json::Value;
use std::io::{BufRead, BufReader};
use std::time::Instant;
use tokio::io::AsyncBufReadExt;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Example 1: Streaming events from a query endpoint
    println!("=== Example 1: Basic Streaming Response ===");
    basic_streaming_example().await?;

    println!("\n=== Example 2: Streaming with Backpressure ===");
    streaming_with_backpressure().await?;

    println!("\n=== Example 3: Memory-Efficient Processing ===");
    memory_efficient_processing().await?;

    Ok(())
}

async fn basic_streaming_example() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = "http://localhost:8000/api/v1/events?limit=1000&stream=true";

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;

    let mut count = 0;
    let start = Instant::now();

    if let Ok(body_str) = response.text().await {
        let trimmed = body_str.trim_start_matches('[').trim_end_matches(']');
        for item_str in trimmed.split(',') {
            match serde_json::from_str::<Value>(item_str.trim()) {
                Ok(_value) => {
                    count += 1;
                    if count % 1000 == 0 {
                        println!(
                            "Processed {} items in {:.2}s",
                            count,
                            start.elapsed().as_secs_f64()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Failed to parse item: {}", e);
                }
            }
        }
    }

    println!(
        "Total: {} items in {:.2}s ({:.0} items/sec)",
        count,
        start.elapsed().as_secs_f64(),
        count as f64 / start.elapsed().as_secs_f64()
    );

    Ok(())
}

async fn streaming_with_backpressure() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = "http://localhost:8000/api/v1/events?stream=true";

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;

    let mut count = 0;
    let mut batch = Vec::with_capacity(100);
    let start = Instant::now();

    if let Ok(body_str) = response.text().await {
        let trimmed = body_str.trim_start_matches('[').trim_end_matches(']');
        for item_str in trimmed.split(',') {
            if let Ok(value) = serde_json::from_str::<Value>(item_str.trim()) {
                batch.push(value);
                count += 1;

                if batch.len() >= 100 {
                    process_batch(&batch).await?;
                    batch.clear();
                }
            }
        }

        if !batch.is_empty() {
            process_batch(&batch).await?;
        }
    }

    println!(
        "Processed {} items in batches of 100 in {:.2}s",
        count,
        start.elapsed().as_secs_f64()
    );

    Ok(())
}

async fn memory_efficient_processing() -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = "http://localhost:8000/api/v1/events?stream=true&batch_size=1000";

    let response = client
        .get(url)
        .header("Accept", "application/json")
        .send()
        .await?;

    let start = Instant::now();
    let mut count = 0u64;
    let mut max_memory_mb = 0.0;

    if let Ok(body_str) = response.text().await {
        let trimmed = body_str.trim_start_matches('[').trim_end_matches(']');

        for (idx, item_str) in trimmed.split(',').enumerate() {
            match serde_json::from_str::<Value>(item_str.trim()) {
                Ok(value) => {
                    process_single_event(&value).await?;
                    count += 1;

                    // Check memory periodically
                    if idx % 10000 == 0 {
                        let current_memory = get_current_memory_mb();
                        if current_memory > max_memory_mb {
                            max_memory_mb = current_memory;
                        }

                        println!(
                            "Processed {} items, current memory: {:.2}MB, elapsed: {:.2}s",
                            count,
                            current_memory,
                            start.elapsed().as_secs_f64()
                        );
                    }
                }
                Err(e) => {
                    eprintln!("Parse error: {}", e);
                }
            }
        }
    }

    println!(
        "Final: {} items processed",
        count
    );
    println!(
        "Max memory: {:.2}MB",
        max_memory_mb
    );
    println!(
        "Total time: {:.2}s ({:.0} items/sec)",
        start.elapsed().as_secs_f64(),
        count as f64 / start.elapsed().as_secs_f64()
    );

    Ok(())
}

async fn process_batch(batch: &[Value]) -> Result<(), Box<dyn std::error::Error>> {
    // Simulate batch processing (e.g., database insert, aggregation)
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
    Ok(())
}

async fn process_single_event(_event: &Value) -> Result<(), Box<dyn std::error::Error>> {
    // Process individual event
    Ok(())
}

fn get_current_memory_mb() -> f64 {
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            if let Some(line) = status.lines().find(|l| l.starts_with("VmRSS:")) {
                if let Some(rss_str) = line.split_whitespace().nth(1) {
                    if let Ok(rss_kb) = rss_str.parse::<f64>() {
                        return rss_kb / 1024.0;
                    }
                }
            }
        }
    }
    0.0
}

// Example: Using streaming with filtering
async fn streaming_with_filtering(
    contract_id: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::new();
    let url = format!(
        "http://localhost:8000/api/v1/events?contract_id={}&stream=true",
        contract_id
    );

    let response = client
        .get(&url)
        .header("Accept", "application/json")
        .send()
        .await?;

    if let Ok(body_str) = response.text().await {
        let trimmed = body_str.trim_start_matches('[').trim_end_matches(']');
        for item_str in trimmed.split(',') {
            if let Ok(value) = serde_json::from_str::<Value>(item_str.trim()) {
                if let Some(contract) = value.get("contract_id").and_then(|v| v.as_str()) {
                    if contract == contract_id {
                        println!("Event: {:?}", value);
                    }
                }
            }
        }
    }

    Ok(())
}
