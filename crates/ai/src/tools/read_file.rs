//! Read file tool - read contents of files passed by the user.
//!
//! This tool allows the AI assistant to read file contents from paths
//! provided in the conversation. Supports text files, CSV, JSON, etc.

use rig::{completion::ToolDefinition, tool::Tool};
use serde::{Deserialize, Serialize};
use std::path::Path;
use log::debug;

use crate::error::AiError;

// ============================================================================
// Constants
// ============================================================================

/// Maximum file size to read (5 MB)
const MAX_FILE_SIZE: u64 = 5 * 1024 * 1024;

/// Maximum content length to return (100 KB to avoid context overflow)
const MAX_CONTENT_LENGTH: usize = 100 * 1024;

/// Allowed file extensions for safety
const ALLOWED_EXTENSIONS: &[&str] = &[
    // Text files
    "txt", "md", "markdown", "rst", "log",
    // Data files
    "csv", "tsv", "json", "jsonl", "xml", "yaml", "yml", "toml",
    // Code files
    "py", "rs", "js", "ts", "jsx", "tsx", "html", "css", "scss",
    "sql", "sh", "bash", "ps1", "bat", "cmd",
    // Config files
    "ini", "cfg", "conf", "env", "properties",
    // Other
    "pdf", // Will show warning for binary
];

// ============================================================================
// Tool Arguments and Output
// ============================================================================

/// Arguments for the read_file tool.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileArgs {
    /// Path to the file to read (absolute or relative)
    pub file_path: String,
    
    /// Optional: starting line number (1-indexed, default: 1)
    #[serde(default)]
    pub start_line: Option<usize>,
    
    /// Optional: ending line number (1-indexed, inclusive)
    #[serde(default)]
    pub end_line: Option<usize>,
    
    /// Optional: encoding hint (default: utf-8)
    #[serde(default)]
    pub encoding: Option<String>,
}

/// Output envelope for read_file tool.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadFileOutput {
    /// Whether the file was read successfully
    pub success: bool,
    
    /// File path that was read
    pub file_path: String,
    
    /// File name only
    pub file_name: String,
    
    /// File extension
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extension: Option<String>,
    
    /// File size in bytes
    pub file_size: u64,
    
    /// Content of the file (text)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    
    /// Total number of lines in the file
    pub total_lines: usize,
    
    /// Lines returned (if range specified)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lines_returned: Option<String>,
    
    /// Whether output was truncated
    #[serde(default)]
    pub truncated: bool,
    
    /// Error message if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    
    /// Warning message (e.g., binary file detected)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}

// ============================================================================
// Tool Implementation
// ============================================================================

/// Tool to read file contents.
#[derive(Clone)]
pub struct ReadFileTool;

impl ReadFileTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReadFileTool {
    fn default() -> Self {
        Self::new()
    }
}

impl Tool for ReadFileTool {
    const NAME: &'static str = "read_file";

    type Error = AiError;
    type Args = ReadFileArgs;
    type Output = ReadFileOutput;

    async fn definition(&self, _prompt: String) -> ToolDefinition {
        ToolDefinition {
            name: Self::NAME.to_string(),
            description: "Read the contents of a file. Use this to analyze files the user provides or references. Supports text files, CSV, JSON, code files, etc. Returns file content as text.".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "filePath": {
                        "type": "string",
                        "description": "Path to the file to read. Can be absolute or relative path."
                    },
                    "startLine": {
                        "type": "integer",
                        "description": "Optional: starting line number (1-indexed). Use for large files to read specific sections.",
                        "minimum": 1
                    },
                    "endLine": {
                        "type": "integer", 
                        "description": "Optional: ending line number (1-indexed, inclusive). Use with startLine to read specific sections.",
                        "minimum": 1
                    },
                    "encoding": {
                        "type": "string",
                        "description": "Optional: encoding hint (default: utf-8). Common values: utf-8, latin-1, utf-16.",
                        "default": "utf-8"
                    }
                },
                "required": ["filePath"]
            }),
        }
    }

    async fn call(&self, args: Self::Args) -> Result<Self::Output, Self::Error> {
        debug!("ReadFileTool: reading file {:?}", args.file_path);
        
        let path = Path::new(&args.file_path);
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());
        let extension = path
            .extension()
            .map(|s| s.to_string_lossy().to_lowercase());

        // Check if file exists
        if !path.exists() {
            return Ok(ReadFileOutput {
                success: false,
                file_path: args.file_path,
                file_name,
                extension,
                file_size: 0,
                content: None,
                total_lines: 0,
                lines_returned: None,
                truncated: false,
                error: Some("File not found".to_string()),
                warning: None,
            });
        }

        // Check if it's a directory
        if path.is_dir() {
            return Ok(ReadFileOutput {
                success: false,
                file_path: args.file_path,
                file_name,
                extension,
                file_size: 0,
                content: None,
                total_lines: 0,
                lines_returned: None,
                truncated: false,
                error: Some("Path is a directory, not a file".to_string()),
                warning: None,
            });
        }

        // Get file metadata
        let metadata = std::fs::metadata(path)
            .map_err(|e| AiError::ToolExecutionFailed(format!("Cannot read file metadata: {}", e)))?;
        let file_size = metadata.len();

        // Check file size
        if file_size > MAX_FILE_SIZE {
            return Ok(ReadFileOutput {
                success: false,
                file_path: args.file_path,
                file_name,
                extension,
                file_size,
                content: None,
                total_lines: 0,
                lines_returned: None,
                truncated: false,
                error: Some(format!(
                    "File too large: {} bytes (max: {} bytes / {} MB)",
                    file_size, MAX_FILE_SIZE, MAX_FILE_SIZE / 1024 / 1024
                )),
                warning: None,
            });
        }

        // Check extension (warning only, not blocking)
        let mut warning = None;
        if let Some(ref ext) = extension {
            if !ALLOWED_EXTENSIONS.contains(&ext.as_str()) {
                warning = Some(format!(
                    "File extension '.{}' is not in the recommended list. Content may be binary or unreadable.",
                    ext
                ));
            }
        }

        // Read file content
        let content_result = std::fs::read_to_string(path);
        
        let content = match content_result {
            Ok(c) => c,
            Err(e) => {
                // Try reading as bytes and convert with lossy UTF-8
                match std::fs::read(path) {
                    Ok(bytes) => {
                        warning = Some("File contains non-UTF-8 characters. Some content may be replaced with '�'.".to_string());
                        String::from_utf8_lossy(&bytes).to_string()
                    }
                    Err(_) => {
                        return Ok(ReadFileOutput {
                            success: false,
                            file_path: args.file_path,
                            file_name,
                            extension,
                            file_size,
                            content: None,
                            total_lines: 0,
                            lines_returned: None,
                            truncated: false,
                            error: Some(format!("Cannot read file: {}", e)),
                            warning,
                        });
                    }
                }
            }
        };

        // Split into lines
        let lines: Vec<&str> = content.lines().collect();
        let total_lines = lines.len();

        // Apply line range if specified
        let (filtered_content, lines_returned, truncated) = if args.start_line.is_some() || args.end_line.is_some() {
            let start = args.start_line.unwrap_or(1).saturating_sub(1); // Convert to 0-indexed
            let end = args.end_line.unwrap_or(total_lines).min(total_lines);
            
            if start >= total_lines {
                return Ok(ReadFileOutput {
                    success: false,
                    file_path: args.file_path,
                    file_name,
                    extension,
                    file_size,
                    content: None,
                    total_lines,
                    lines_returned: None,
                    truncated: false,
                    error: Some(format!(
                        "Start line {} exceeds total lines {}",
                        start + 1, total_lines
                    )),
                    warning,
                });
            }

            let selected_lines: Vec<&str> = lines[start..end].to_vec();
            let mut result = selected_lines.join("\n");
            let mut is_truncated = false;

            // Truncate if still too long
            if result.len() > MAX_CONTENT_LENGTH {
                result.truncate(MAX_CONTENT_LENGTH);
                is_truncated = true;
            }

            (
                result,
                Some(format!("{}-{}", start + 1, end)),
                is_truncated,
            )
        } else {
            // Return full content (with truncation if needed)
            let mut result = content;
            let is_truncated = result.len() > MAX_CONTENT_LENGTH;
            
            if is_truncated {
                result.truncate(MAX_CONTENT_LENGTH);
            }

            (result, None, is_truncated)
        };

        Ok(ReadFileOutput {
            success: true,
            file_path: args.file_path,
            file_name,
            extension,
            file_size,
            content: Some(filtered_content),
            total_lines,
            lines_returned,
            truncated,
            error: None,
            warning,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_read_nonexistent_file() {
        let tool = ReadFileTool::new();
        let result = tool.call(ReadFileArgs {
            file_path: "/nonexistent/file.txt".to_string(),
            start_line: None,
            end_line: None,
            encoding: None,
        }).await.unwrap();

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.error.unwrap().contains("not found"));
    }
}
