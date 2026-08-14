// HalluMeter — llamabridge `bridge.yaml` model context for Continue fill %.

use super::continue_types::{continue_normalize_model_id, ContinueModelConfig};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct BridgeYamlFile {
    #[serde(default)]
    models: HashMap<String, BridgeModelBlock>,
}

#[derive(Debug, Deserialize)]
struct BridgeModelBlock {
    num_ctx: Option<u64>,
}

/// Builds the same map shape as `~/.continue/config.yaml` parsing, using each
/// `models.<id>.num_ctx` from a bridge-style YAML (e.g. llamabridge `config/bridge.yaml`).
pub(crate) fn bridge_model_config_map_from_path(
    path: &Path,
) -> Result<HashMap<String, ContinueModelConfig>, String> {
    let mut map = HashMap::new();
    let content = fs::read_to_string(path)
        .map_err(|error| format!("Could not read {}: {error}", path.display()))?;
    let file = serde_yaml::from_str::<BridgeYamlFile>(&content)
        .map_err(|error| format!("Invalid Continue bridge {}: {error}", path.display()))?;

    for (raw_id, block) in file.models {
        let Some(context_window) = block.num_ctx.filter(|n| *n > 0) else {
            continue;
        };
        let key = continue_normalize_model_id(&raw_id);
        if key.is_empty() {
            continue;
        }
        let config = ContinueModelConfig {
            canonical_model_id: key.clone(),
            context_window,
        };
        map.insert(key, config);
    }

    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_yaml_num_ctx_fills_model_map() {
        let path = std::env::temp_dir().join("hallueter-bridge-yaml-test.yaml");
        let _ = std::fs::remove_file(&path);
        std::fs::write(
            &path,
            b"models:\n  qwen36-27b-q3km:\n    num_ctx: 98304\n    gguf_path: /dev/null\n",
        )
        .unwrap();
        let m = bridge_model_config_map_from_path(&path).expect("parses bridge yaml");
        let _ = std::fs::remove_file(&path);
        let entry = m.get("qwen36-27b-q3km").expect("key from yaml");
        assert_eq!(entry.context_window, 98304);
        assert_eq!(entry.canonical_model_id, "qwen36-27b-q3km");
    }
}
