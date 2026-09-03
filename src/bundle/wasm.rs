use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use wasmparser::{Parser, Payload};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WasmSectionInfo {
    pub name: String,
    pub section_type: String,
    pub size_bytes: usize,
    pub percentage: f64,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WasmAnalysis {
    pub file_path: String,
    pub total_size_bytes: usize,
    pub code_section_bytes: usize,
    pub data_section_bytes: usize,
    pub debug_symbol_bytes: usize,
    pub debug_symbol_percentage: f64,
    pub sections: Vec<WasmSectionInfo>,
    pub export_count: usize,
    pub import_count: usize,
    pub function_count: usize,
    pub exported_functions: Vec<String>,
    pub producers: Vec<String>,
    pub recommendations: Vec<String>,
}

pub fn analyze_wasm_bytes(file_path: &str, data: &[u8]) -> Result<WasmAnalysis, String> {
    let total_size_bytes = data.len();
    if total_size_bytes < 8 {
        return Err("File too small to be a valid WebAssembly module".to_string());
    }

    // WASM magic bytes: \0asm, version 1
    if &data[0..4] != b"\0asm" {
        return Err("Invalid WebAssembly magic number".to_string());
    }

    let mut sections: Vec<WasmSectionInfo> = Vec::new();
    let mut code_section_bytes = 0usize;
    let mut data_section_bytes = 0usize;
    let mut debug_symbol_bytes = 0usize;
    let mut export_count = 0usize;
    let mut import_count = 0usize;
    let mut function_count = 0usize;
    let mut exported_functions = Vec::new();
    let mut producers = Vec::new();

    let parser = Parser::new(0);
    for payload in parser.parse_all(data) {
        let payload = payload.map_err(|e| format!("WASM parse error: {e}"))?;
        match payload {
            Payload::Version { range, .. } => {
                // Header (8 bytes)
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Header".to_string(),
                    section_type: "Header".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some("WASM Magic & Version (8 bytes)".to_string()),
                });
            }
            Payload::TypeSection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Type".to_string(),
                    section_type: "Type".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} function signatures")),
                });
            }
            Payload::ImportSection(reader) => {
                let count = reader.count();
                import_count = count as usize;
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Import".to_string(),
                    section_type: "Import".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} imports")),
                });
            }
            Payload::FunctionSection(reader) => {
                let count = reader.count();
                function_count = count as usize;
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Function".to_string(),
                    section_type: "Function".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} function declarations")),
                });
            }
            Payload::TableSection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Table".to_string(),
                    section_type: "Table".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} tables")),
                });
            }
            Payload::MemorySection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Memory".to_string(),
                    section_type: "Memory".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} memories")),
                });
            }
            Payload::GlobalSection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Global".to_string(),
                    section_type: "Global".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} globals")),
                });
            }
            Payload::ExportSection(reader) => {
                let count = reader.count();
                export_count = count as usize;
                let range = reader.range();
                let size = range.end - range.start;

                for export in reader {
                    if let Ok(exp) = export
                        && exported_functions.len() < 25
                    {
                        exported_functions.push(format!("{} ({:?})", exp.name, exp.kind));
                    }
                }

                sections.push(WasmSectionInfo {
                    name: "Export".to_string(),
                    section_type: "Export".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} exports")),
                });
            }
            Payload::StartSection { func, range } => {
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Start".to_string(),
                    section_type: "Start".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("Start func idx: {func}")),
                });
            }
            Payload::ElementSection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "Element".to_string(),
                    section_type: "Element".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} elements")),
                });
            }
            Payload::CodeSectionStart { count, range, .. } => {
                let size = range.end - range.start;
                code_section_bytes = size;
                sections.push(WasmSectionInfo {
                    name: "Code".to_string(),
                    section_type: "Code".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} function bodies")),
                });
            }
            Payload::DataSection(reader) => {
                let count = reader.count();
                let range = reader.range();
                let size = range.end - range.start;
                data_section_bytes = size;
                sections.push(WasmSectionInfo {
                    name: "Data".to_string(),
                    section_type: "Data".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("{count} data segments (static memory/strings)")),
                });
            }
            Payload::DataCountSection { count, range } => {
                let size = range.end - range.start;
                sections.push(WasmSectionInfo {
                    name: "DataCount".to_string(),
                    section_type: "DataCount".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details: Some(format!("count: {count}")),
                });
            }
            Payload::CustomSection(reader) => {
                let name = reader.name();
                let range = reader.range();
                let size = range.end - range.start;

                let is_debug = name == "name" || name.starts_with(".debug_") || name == "producers";

                if is_debug {
                    debug_symbol_bytes += size;
                }

                if name == "producers" {
                    producers.push(format!("producers ({} bytes)", size));
                }

                let details = if name == "name" {
                    Some("Function & local names (debug symbols)".to_string())
                } else if name.starts_with(".debug_") {
                    Some("DWARF debug information".to_string())
                } else if name == "producers" {
                    Some("Compiler / toolchain metadata".to_string())
                } else {
                    Some(format!("Custom section: {name}"))
                };

                sections.push(WasmSectionInfo {
                    name: format!("Custom({name})"),
                    section_type: "Custom".to_string(),
                    size_bytes: size,
                    percentage: (size as f64 / total_size_bytes as f64) * 100.0,
                    details,
                });
            }
            _ => {}
        }
    }

    let debug_symbol_percentage = if total_size_bytes > 0 {
        (debug_symbol_bytes as f64 / total_size_bytes as f64) * 100.0
    } else {
        0.0
    };

    // Formulate actionable optimization recommendations
    let mut recommendations = Vec::new();

    if debug_symbol_bytes > 1024 * 10 || debug_symbol_percentage > 5.0 {
        recommendations.push(format!(
            "Strip debug symbols to save {} ({:.1}% of binary). Run: wasm-opt -O3 --strip-debug {} -o {}",
            crate::bundle::budget::format_bytes(debug_symbol_bytes),
            debug_symbol_percentage,
            file_path,
            file_path
        ));
    }

    if data_section_bytes as f64 / total_size_bytes as f64 > 0.35 {
        recommendations.push(
            "Large Data Section detected (>35% of binary). Check for embedded assets or uncompressed lookup tables."
                .to_string(),
        );
    }

    if code_section_bytes as f64 / total_size_bytes as f64 > 0.70 {
        recommendations.push(
            "Large Code Section detected (>70% of binary). Run wasm-opt -Oz to apply aggressive dead code elimination and function inlining tuning."
                .to_string(),
        );
    }

    if recommendations.is_empty() {
        recommendations.push("WebAssembly binary is well-optimized.".to_string());
    }

    Ok(WasmAnalysis {
        file_path: file_path.to_string(),
        total_size_bytes,
        code_section_bytes,
        data_section_bytes,
        debug_symbol_bytes,
        debug_symbol_percentage,
        sections,
        export_count,
        import_count,
        function_count,
        exported_functions,
        producers,
        recommendations,
    })
}

pub fn analyze_wasm_file(path: &Path) -> Result<WasmAnalysis, String> {
    let bytes =
        fs::read(path).map_err(|e| format!("Failed to read WASM file {}: {e}", path.display()))?;
    analyze_wasm_bytes(&path.to_string_lossy(), &bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wasm_analysis_synthetic() {
        // Minimal valid WASM binary: \0asm (magic) + \x01\x00\x00\x00 (version 1)
        let mut wasm_bytes = vec![0x00, 0x61, 0x73, 0x6D, 0x01, 0x00, 0x00, 0x00];

        // Add a custom section named "name" (id 0)
        // section id 0, length 6, name length 4, "name", data 1
        wasm_bytes.extend_from_slice(&[0x00, 0x06, 0x04, b'n', b'a', b'm', b'e', 0x00]);

        let analysis = analyze_wasm_bytes("test.wasm", &wasm_bytes).expect("wasm analysis failed");
        assert_eq!(analysis.total_size_bytes, wasm_bytes.len());
        assert!(analysis.debug_symbol_bytes > 0);
        assert!(!analysis.sections.is_empty());
    }
}
