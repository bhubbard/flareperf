use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceMapData {
    pub version: Option<u32>,
    pub file: Option<String>,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub sources_content: Option<Vec<Option<String>>>,
    #[serde(default)]
    pub names: Vec<String>,
    pub mappings: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PackageContribution {
    pub package_name: String,
    pub byte_count: usize,
    pub percentage: f64,
    pub file_count: usize,
    pub sample_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SourceContribution {
    pub source_path: String,
    pub package_name: String,
    pub byte_count: usize,
    pub percentage: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourcemapAnalysis {
    pub sourcemap_path: String,
    pub generated_file_path: Option<String>,
    pub total_generated_bytes: usize,
    pub total_mapped_bytes: usize,
    pub total_unmapped_bytes: usize,
    pub mapped_percentage: f64,
    pub packages: Vec<PackageContribution>,
    pub top_sources: Vec<SourceContribution>,
    pub top_packages: Vec<PackageContribution>,
}

#[derive(Debug, Clone, Copy)]
struct MappingSegment {
    gen_col: usize,
    src_idx: Option<usize>,
    _src_line: Option<usize>,
    _src_col: Option<usize>,
    _name_idx: Option<usize>,
}

pub fn decode_vlq_char(c: char) -> Result<i64, String> {
    match c {
        'A'..='Z' => Ok((c as u8 - b'A') as i64),
        'a'..='z' => Ok((c as u8 - b'a' + 26) as i64),
        '0'..='9' => Ok((c as u8 - b'0' + 52) as i64),
        '+' => Ok(62),
        '/' => Ok(63),
        _ => Err(format!("Invalid Base64 character in VLQ: '{c}'")),
    }
}

pub fn decode_vlq_sequence(s: &str) -> Result<Vec<i64>, String> {
    let mut values = Vec::new();
    let mut shift = 0;
    let mut value = 0i64;

    for c in s.chars() {
        let digit = decode_vlq_char(c)?;
        let has_continuation = (digit & 32) != 0;
        let v = digit & 31;
        value += v << shift;

        if has_continuation {
            shift += 5;
        } else {
            let is_negative = (value & 1) != 0;
            let final_val = value >> 1;
            values.push(if is_negative { -final_val } else { final_val });
            value = 0;
            shift = 0;
        }
    }

    Ok(values)
}

pub fn extract_package_name(source_path: &str) -> String {
    let normalized = source_path.replace('\\', "/");

    // Check for node_modules
    if let Some(idx) = normalized.find("node_modules/") {
        let after_nm = &normalized[idx + "node_modules/".len()..];
        let parts: Vec<&str> = after_nm.split('/').filter(|s| !s.is_empty()).collect();
        if !parts.is_empty() {
            if parts[0].starts_with('@') && parts.len() > 1 {
                return format!("{}/{}", parts[0], parts[1]);
            } else {
                return parts[0].to_string();
            }
        }
    }

    // Check for webpack runtime / internal
    if normalized.starts_with("webpack/") || normalized.contains("/webpack/") {
        return "[webpack runtime]".to_string();
    }

    if normalized.starts_with("turbopack/") || normalized.contains("/turbopack/") {
        return "[turbopack runtime]".to_string();
    }

    if normalized.starts_with("esbuild/") || normalized.contains("/esbuild/") {
        return "[esbuild runtime]".to_string();
    }

    // App source
    "[app source]".to_string()
}

pub fn analyze_sourcemap(
    map_path: &Path,
    generated_content_override: Option<&str>,
) -> Result<SourcemapAnalysis, String> {
    let map_content = fs::read_to_string(map_path)
        .map_err(|e| format!("Failed to read sourcemap {}: {e}", map_path.display()))?;

    let map_data: SourceMapData = serde_json::from_str(&map_content)
        .map_err(|e| format!("Failed to parse sourcemap JSON {}: {e}", map_path.display()))?;

    // Determine generated file
    let generated_file_path = if let Some(ref f) = map_data.file {
        let parent = map_path.parent().unwrap_or_else(|| Path::new("."));
        let candidate = parent.join(f);
        if candidate.exists() {
            Some(candidate)
        } else {
            None
        }
    } else {
        // Try stripping .map
        let map_name = map_path.to_string_lossy();
        if let Some(base) = map_name.strip_suffix(".map") {
            let p = PathBuf::from(base);
            if p.exists() { Some(p) } else { None }
        } else {
            None
        }
    };

    let generated_content = if let Some(content) = generated_content_override {
        content.to_string()
    } else if let Some(ref gen_path) = generated_file_path {
        fs::read_to_string(gen_path).unwrap_or_default()
    } else {
        String::new()
    };

    let gen_lines: Vec<&str> = if !generated_content.is_empty() {
        generated_content.lines().collect()
    } else {
        Vec::new()
    };

    let mut source_byte_counts: HashMap<usize, usize> = HashMap::new();
    let mut unmapped_bytes = 0usize;
    let mut total_generated_bytes = generated_content.len();

    let mut prev_src_idx: i64 = 0;
    let mut prev_src_line: i64 = 0;
    let mut prev_src_col: i64 = 0;
    let mut prev_name_idx: i64 = 0;

    let map_lines: Vec<&str> = map_data.mappings.split(';').collect();

    for (line_idx, map_line) in map_lines.iter().enumerate() {
        let gen_line_len = if line_idx < gen_lines.len() {
            gen_lines[line_idx].len()
        } else {
            0
        };

        if map_line.is_empty() {
            unmapped_bytes += gen_line_len;
            continue;
        }

        let mut segments: Vec<MappingSegment> = Vec::new();
        let mut prev_gen_col: i64 = 0;

        for seg_str in map_line.split(',') {
            if seg_str.is_empty() {
                continue;
            }

            let nums = decode_vlq_sequence(seg_str)?;
            if nums.is_empty() {
                continue;
            }

            let gen_col_delta = nums[0];
            let gen_col = (prev_gen_col + gen_col_delta).max(0) as usize;
            prev_gen_col += gen_col_delta;

            let mut src_idx = None;
            let mut src_line = None;
            let mut src_col = None;
            let mut name_idx = None;

            if nums.len() >= 4 {
                prev_src_idx += nums[1];
                prev_src_line += nums[2];
                prev_src_col += nums[3];

                if prev_src_idx >= 0 && (prev_src_idx as usize) < map_data.sources.len() {
                    src_idx = Some(prev_src_idx as usize);
                }
                src_line = Some(prev_src_line.max(0) as usize);
                src_col = Some(prev_src_col.max(0) as usize);
            }

            if nums.len() >= 5 {
                prev_name_idx += nums[4];
                name_idx = Some(prev_name_idx.max(0) as usize);
            }

            segments.push(MappingSegment {
                gen_col,
                src_idx,
                _src_line: src_line,
                _src_col: src_col,
                _name_idx: name_idx,
            });
        }

        if segments.is_empty() {
            unmapped_bytes += gen_line_len;
            continue;
        }

        // Unmapped prefix
        if segments[0].gen_col > 0 {
            unmapped_bytes += segments[0].gen_col.min(gen_line_len);
        }

        for i in 0..segments.len() {
            let start = segments[i].gen_col;
            let end = if i + 1 < segments.len() {
                segments[i + 1].gen_col
            } else {
                gen_line_len.max(start)
            };

            let byte_count = end.saturating_sub(start);

            if let Some(s_idx) = segments[i].src_idx {
                *source_byte_counts.entry(s_idx).or_insert(0) += byte_count;
            } else {
                unmapped_bytes += byte_count;
            }
        }
    }

    let total_mapped_bytes: usize = source_byte_counts.values().sum();
    if total_generated_bytes == 0 {
        total_generated_bytes = total_mapped_bytes + unmapped_bytes;
    }

    // Aggregate by package and source file
    let mut pkg_map: HashMap<String, (usize, Vec<String>)> = HashMap::new();
    let mut top_sources: Vec<SourceContribution> = Vec::new();

    for (s_idx, bytes) in source_byte_counts {
        let src_path = map_data
            .sources
            .get(s_idx)
            .cloned()
            .unwrap_or_else(|| format!("source_{s_idx}"));
        let pkg_name = extract_package_name(&src_path);

        let pct = if total_generated_bytes > 0 {
            (bytes as f64 / total_generated_bytes as f64) * 100.0
        } else {
            0.0
        };

        top_sources.push(SourceContribution {
            source_path: src_path.clone(),
            package_name: pkg_name.clone(),
            byte_count: bytes,
            percentage: pct,
        });

        let entry = pkg_map.entry(pkg_name).or_insert((0, Vec::new()));
        entry.0 += bytes;
        if entry.1.len() < 5 {
            entry.1.push(src_path);
        }
    }

    top_sources.sort_by_key(|b| std::cmp::Reverse(b.byte_count));

    let mut packages: Vec<PackageContribution> = pkg_map
        .into_iter()
        .map(|(pkg, (bytes, samples))| {
            let pct = if total_generated_bytes > 0 {
                (bytes as f64 / total_generated_bytes as f64) * 100.0
            } else {
                0.0
            };
            PackageContribution {
                package_name: pkg,
                byte_count: bytes,
                percentage: pct,
                file_count: samples.len(),
                sample_files: samples,
            }
        })
        .collect();

    packages.sort_by_key(|b| std::cmp::Reverse(b.byte_count));

    let mapped_percentage = if total_generated_bytes > 0 {
        (total_mapped_bytes as f64 / total_generated_bytes as f64) * 100.0
    } else {
        0.0
    };

    let top_packages = packages.iter().take(10).cloned().collect();

    Ok(SourcemapAnalysis {
        sourcemap_path: map_path.to_string_lossy().to_string(),
        generated_file_path: generated_file_path.map(|p| p.to_string_lossy().to_string()),
        total_generated_bytes,
        total_mapped_bytes,
        total_unmapped_bytes: unmapped_bytes,
        mapped_percentage,
        top_packages,
        packages,
        top_sources,
    })
}

pub fn find_sourcemap_for_file(script_path: &Path) -> Option<PathBuf> {
    // 1. Check adjacent .map
    let mut candidate = script_path.to_path_buf();
    candidate.set_extension(format!(
        "{}.map",
        script_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("js")
    ));
    if candidate.exists() {
        return Some(candidate);
    }

    let candidate_direct = PathBuf::from(format!("{}.map", script_path.display()));
    if candidate_direct.exists() {
        return Some(candidate_direct);
    }

    // 2. Scan file content for sourceMappingURL
    if let Ok(content) = fs::read_to_string(script_path) {
        for line in content.lines().rev().take(10) {
            if let Some(idx) = line.find("//# sourceMappingURL=") {
                let url = line[idx + "//# sourceMappingURL=".len()..].trim();
                if !url.starts_with("data:") {
                    let parent = script_path.parent().unwrap_or_else(|| Path::new("."));
                    let resolved = parent.join(url);
                    if resolved.exists() {
                        return Some(resolved);
                    }
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vlq_decoding() {
        assert_eq!(decode_vlq_sequence("AAAA").unwrap(), vec![0, 0, 0, 0]);
        assert_eq!(decode_vlq_sequence("CAAC").unwrap(), vec![1, 0, 0, 1]);
        assert_eq!(decode_vlq_sequence("EAAE").unwrap(), vec![2, 0, 0, 2]);
    }

    #[test]
    fn test_package_name_extraction() {
        assert_eq!(
            extract_package_name("node_modules/lodash/debounce.js"),
            "lodash"
        );
        assert_eq!(
            extract_package_name("node_modules/@cloudflare/workers-types/index.ts"),
            "@cloudflare/workers-types"
        );
        assert_eq!(
            extract_package_name("../node_modules/itty-router/dist/index.mjs"),
            "itty-router"
        );
        assert_eq!(extract_package_name("src/handlers/api.ts"), "[app source]");
        assert_eq!(
            extract_package_name("webpack/runtime/bootstrap"),
            "[webpack runtime]"
        );
    }

    #[test]
    fn test_sourcemap_analysis_synthetic() {
        let temp_dir = std::env::temp_dir();
        let map_file = temp_dir.join("test_bundle.js.map");
        let gen_file = temp_dir.join("test_bundle.js");

        let generated_js = "function hello() { return 'world'; }\nconsole.log(hello());\n";
        let map_json = serde_json::json!({
            "version": 3,
            "file": "test_bundle.js",
            "sources": [
                "node_modules/lodash/index.js",
                "src/index.ts"
            ],
            "names": [],
            "mappings": "AAAA,SAAS,CAAC;ACAV,OAAO,CAAC"
        });

        fs::write(&map_file, serde_json::to_string(&map_json).unwrap()).unwrap();
        fs::write(&gen_file, generated_js).unwrap();

        let analysis = analyze_sourcemap(&map_file, Some(generated_js)).expect("analysis failed");
        assert_eq!(analysis.packages.len(), 2);
        assert!(analysis.total_mapped_bytes > 0);

        let _ = fs::remove_file(map_file);
        let _ = fs::remove_file(gen_file);
    }
}
