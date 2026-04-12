// Simple HTTP server to receive stats from remote ctk instances
use anyhow::Result;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use crate::stats::Stats;

pub fn run_stats_server(port: u16) -> Result<()> {
    let listener = TcpListener::bind(format!("0.0.0.0:{}", port))?;
    eprintln!("[ctk stats-server] listening on port {}", port);
    
    for stream in listener.incoming() {
        if let Ok(stream) = stream {
            handle_client(stream).ok();
        }
    }
    
    Ok(())
}

fn handle_client(mut stream: TcpStream) -> Result<()> {
    let buf_reader = BufReader::new(&stream);
    let mut lines = buf_reader.lines();
    
    // Read request line
    let _request_line = lines.next().unwrap_or(Ok(String::new()))?;
    
    // Skip headers
    let mut content_length = 0;
    for line in lines {
        let line = line?;
        if line.is_empty() {
            break;
        }
        if line.starts_with("Content-Length:") {
            content_length = line.split(':').nth(1)
                .and_then(|s| s.trim().parse().ok())
                .unwrap_or(0);
        }
    }
    
    // Read body
    if content_length > 0 {
        let mut body = vec![0u8; content_length];
        stream.read_exact(&mut body)?;
        
        // Parse and merge stats
        if let Ok(remote_stats) = serde_json::from_slice::<Stats>(&body) {
            let mut local_stats = Stats::load();
            merge_stats(&mut local_stats, &remote_stats);
            local_stats.save()?;
            
            // Send response
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
            stream.write_all(response.as_bytes())?;
            return Ok(());
        }
    }
    
    // Send error response
    let response = "HTTP/1.1 400 Bad Request\r\nContent-Length: 5\r\n\r\nError";
    stream.write_all(response.as_bytes())?;
    Ok(())
}

fn merge_stats(local: &mut Stats, remote: &Stats) {
    local.total_commands += remote.total_commands;
    local.total_raw_tokens += remote.total_raw_tokens;
    local.total_filtered_tokens += remote.total_filtered_tokens;
    local.total_fallbacks += remote.total_fallbacks;
    local.total_chunks += remote.total_chunks;
    
    for (cmd, count) in &remote.command_counts {
        *local.command_counts.entry(cmd.clone()).or_insert(0) += count;
    }
    
    local.recent_events.extend(remote.recent_events.clone());
    
    if remote.last_ai_cli.is_some() {
        local.last_ai_cli = remote.last_ai_cli.clone();
    }
}
