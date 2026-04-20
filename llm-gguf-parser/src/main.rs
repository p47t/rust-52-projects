use anyhow::Result;
use clap::Parser;
use llm_gguf_parser::parse_gguf;
use memmap2::Mmap;
use std::fs::File;
use std::path::Path;

#[derive(Parser, Debug)]
#[command(author, version, about = "A parser for GGUF model files")]
struct Args {
    /// Path to the GGUF model file
    file: String,

    /// Print all tensor names, dimensions and types
    #[arg(short, long)]
    tensors: bool,

    /// Filter metadata keys by a query string
    #[arg(short, long)]
    query: Option<String>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    let file_path = Path::new(&args.file);

    println!("Opening GGUF file: {:?}", file_path);
    let file = File::open(file_path)
        .map_err(|e| anyhow::anyhow!("failed to open file {:?}: {}", file_path, e))?;

    // Memory map the file for extremely fast parsing, which is safe since GGUF structure matches disk.
    let mmap = unsafe { Mmap::map(&file)? };

    println!("File size: {} bytes", mmap.len());
    let gguf =
        parse_gguf(&mmap).map_err(|e| anyhow::anyhow!("failed to parse GGUF file: {}", e))?;

    println!("\n=== GGUF FILE HEADER ===");
    println!("GGUF Version: {}", gguf.version);
    println!("Metadata KV Pairs: {}", gguf.metadata.len());
    println!("Tensors Count: {}", gguf.tensors.len());

    println!("\n=== METADATA ===");
    let mut keys: Vec<&String> = gguf.metadata.keys().collect();
    keys.sort();

    for key in keys {
        if let Some(ref query) = args.query {
            if !key.to_lowercase().contains(&query.to_lowercase()) {
                continue;
            }
        }
        println!("  {}: {}", key, &gguf.metadata[key]);
    }

    if args.tensors {
        println!("\n=== TENSORS ===");
        for (i, tensor) in gguf.tensors.iter().enumerate() {
            let dims = tensor
                .dimensions
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<String>>()
                .join(" × ");
            println!(
                "  [{:>3}] {:<50} | Type: {:<8} | Dims: {:<15} | Offset: {}",
                i, tensor.name, tensor.tensor_type, dims, tensor.offset
            );
        }
    } else {
        println!("\n=== TENSOR SUMMARY (showing first 5, use --tensors to see all) ===");
        for (i, tensor) in gguf.tensors.iter().take(5).enumerate() {
            let dims = tensor
                .dimensions
                .iter()
                .map(|d| d.to_string())
                .collect::<Vec<String>>()
                .join(" × ");
            println!(
                "  [{:>3}] {:<50} | Type: {:<8} | Dims: {:<15}",
                i, tensor.name, tensor.tensor_type, dims
            );
        }
        if gguf.tensors.len() > 5 {
            println!("  ... and {} more tensors", gguf.tensors.len() - 5);
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use llm_gguf_parser::{GgmlType, Value};

    #[test]
    fn test_parse_gguf_mock() {
        // Build a mock GGUF version 3 binary data slice
        let mut data = Vec::new();

        // 1. Magic: 'GGUF' (0x46554747)
        data.extend_from_slice(&0x46554747u32.to_le_bytes());
        // 2. Version: 3
        data.extend_from_slice(&3u32.to_le_bytes());
        // 3. Tensor count: 1
        data.extend_from_slice(&1u64.to_le_bytes());
        // 4. Metadata KV count: 2
        data.extend_from_slice(&2u64.to_le_bytes());

        // Metadata KV Pair 1:
        // Key: "general.architecture" -> length 20
        let key1 = "general.architecture";
        data.extend_from_slice(&(key1.len() as u64).to_le_bytes());
        data.extend_from_slice(key1.as_bytes());
        // Type: String (8)
        data.extend_from_slice(&8u32.to_le_bytes());
        // Value: "llama" -> length 5
        let val1 = "llama";
        data.extend_from_slice(&(val1.len() as u64).to_le_bytes());
        data.extend_from_slice(val1.as_bytes());

        // Metadata KV Pair 2:
        // Key: "llama.context_length" -> length 20
        let key2 = "llama.context_length";
        data.extend_from_slice(&(key2.len() as u64).to_le_bytes());
        data.extend_from_slice(key2.as_bytes());
        // Type: Uint32 (4)
        data.extend_from_slice(&4u32.to_le_bytes());
        // Value: 2048
        data.extend_from_slice(&2048u32.to_le_bytes());

        // Tensor 1:
        // Name: "token_embd.weight" -> length 17
        let t_name = "token_embd.weight";
        data.extend_from_slice(&(t_name.len() as u64).to_le_bytes());
        data.extend_from_slice(t_name.as_bytes());
        // Dimensions count: 2
        data.extend_from_slice(&2u32.to_le_bytes());
        // Dimension 0: 4096
        data.extend_from_slice(&4096u64.to_le_bytes());
        // Dimension 1: 32000
        data.extend_from_slice(&32000u64.to_le_bytes());
        // Tensor type: F16 (1)
        data.extend_from_slice(&1u32.to_le_bytes());
        // Offset: 0
        data.extend_from_slice(&0u64.to_le_bytes());

        let gguf = parse_gguf(&data).unwrap();
        assert_eq!(gguf.version, 3);
        assert_eq!(gguf.metadata.len(), 2);
        assert_eq!(
            gguf.metadata.get("general.architecture"),
            Some(&Value::String("llama".to_string()))
        );
        assert_eq!(
            gguf.metadata.get("llama.context_length"),
            Some(&Value::Uint32(2048))
        );
        assert_eq!(gguf.tensors.len(), 1);
        assert_eq!(gguf.tensors[0].name, "token_embd.weight");
        assert_eq!(gguf.tensors[0].dimensions, vec![4096, 32000]);
        assert_eq!(gguf.tensors[0].tensor_type, GgmlType::F16);
    }
}
