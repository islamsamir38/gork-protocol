//! Agent Skills manifest format
//!
//! Follows the Agent Skills specification from agentskills.io

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use sha2::{Sha256, Digest};

/// Skill manifest (Agent Skills format)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillManifest {
    /// Unique skill name
    pub name: String,

    /// Version string
    pub version: String,

    /// Human-readable description
    pub description: String,

    /// Tags for discovery
    #[serde(default)]
    pub tags: Vec<String>,

    /// Detailed capabilities with schemas
    #[serde(default)]
    pub capabilities: Vec<CapabilityDetail>,

    /// Resource requirements
    #[serde(default)]
    pub requirements: ResourceRequirements,

    /// Optional pricing
    #[serde(rename = "pricing", default)]
    pub pricing: Option<SkillPricing>,

    /// Author (NEAR account)
    pub author: Option<String>,

    /// License
    #[serde(default = "default_license")]
    pub license: String,

    /// Homepage URL
    pub homepage: Option<String>,

    /// Repository URL
    pub repository: Option<String>,
}

fn default_license() -> String {
    "MIT".to_string()
}

/// Detailed capability with JSON schemas
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityDetail {
    /// Capability name
    pub name: String,

    /// Description
    pub description: String,

    /// JSON Schema for input validation
    #[serde(default)]
    pub input_schema: String,

    /// JSON Schema for output validation
    #[serde(default)]
    pub output_schema: String,

    /// Example inputs
    #[serde(default)]
    pub examples: Vec<String>,
}

/// Resource requirements
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ResourceRequirements {
    /// Timeout in seconds
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,

    /// Memory requirement in MB
    #[serde(default = "default_memory")]
    pub memory_mb: u32,

    /// Dependencies (e.g., "python>=3.9")
    #[serde(default)]
    pub dependencies: Vec<String>,
}

fn default_timeout() -> u32 { 30 }
fn default_memory() -> u32 { 512 }

/// Pricing model
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillPricing {
    /// Free tier calls per day
    pub free_tier_calls_per_day: Option<u32>,

    /// Cost per call in yoctoNEAR
    pub cost_per_call_yocto: Option<String>,
}

/// Skill statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillStats {
    pub skill_id: String,
    pub usage_count: u32,
    pub rating: f32,
    pub rating_count: u32,
    pub author: String,
}

/// Skill package (manifest + files)
#[derive(Debug, Clone)]
pub struct SkillPackage {
    /// Manifest
    pub manifest: SkillManifest,

    /// Package directory
    pub root: PathBuf,

    /// IPFS hash (if uploaded)
    pub ipfs_hash: Option<String>,

    /// SHA256 checksum
    pub checksum: Option<String>,
}

impl SkillManifest {
    /// Load from file
    pub fn from_file(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let manifest: SkillManifest = serde_yaml::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse skill.yaml: {}", e))?;

        Ok(manifest)
    }

    /// Validate manifest
    pub fn validate(&self) -> Result<()> {
        // Name validation
        if self.name.is_empty() {
            return Err(anyhow::anyhow!("Skill name cannot be empty"));
        }

        if !self.name.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
            return Err(anyhow::anyhow!("Skill name must be alphanumeric with hyphens/underscores"));
        }

        // Version validation (basic semver)
        if self.version.is_empty() {
            return Err(anyhow::anyhow!("Version cannot be empty"));
        }

        // Description
        if self.description.is_empty() {
            return Err(anyhow::anyhow!("Description cannot be empty"));
        }

        // Tags
        if self.tags.is_empty() {
            return Err(anyhow::anyhow!("At least one tag is required"));
        }

        // Capabilities
        if self.capabilities.is_empty() {
            return Err(anyhow::anyhow!("At least one capability is required"));
        }

        Ok(())
    }

    /// Get skill ID (name@version)
    pub fn skill_id(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

impl SkillPackage {
    /// Load skill package from directory
    pub fn load(path: &Path) -> Result<Self> {
        let root = path.to_path_buf();

        // Look for skill.yaml
        let manifest_path = root.join("skill.yaml");
        if !manifest_path.exists() {
            return Err(anyhow::anyhow!(
                "skill.yaml not found in {}",
                root.display()
            ));
        }

        // Load manifest
        let manifest = SkillManifest::from_file(&manifest_path)?;

        // Compute checksum
        let checksum = Self::compute_checksum(&root)?;

        Ok(Self {
            manifest,
            root,
            ipfs_hash: None,
            checksum: Some(checksum),
        })
    }

    /// Compute SHA256 checksum of package
    fn compute_checksum(path: &Path) -> Result<String> {
        let mut hasher = Sha256::new();

        // Hash manifest
        let manifest_path = path.join("skill.yaml");
        if manifest_path.exists() {
            let content = fs::read(&manifest_path)?;
            hasher.update(&content);
        }

        // Hash code directory if it exists
        let code_dir = path.join("code");
        if code_dir.exists() && code_dir.is_dir() {
            Self::hash_dir(&code_dir, &mut hasher)?;
        }

        Ok(format!("{:x}", hasher.finalize()))
    }

    /// Hash directory recursively
    fn hash_dir(dir: &Path, hasher: &mut Sha256) -> Result<()> {
        let entries = fs::read_dir(dir)?
            .collect::<Result<Vec<_>, _>>()?;

        let mut entries: Vec<_> = entries.into_iter()
            .filter_map(|e| {
                let path = e.path();
                // Skip hidden files and common build artifacts
                let name = path.file_name()?.to_str()?;
                if name.starts_with('.') || name == "node_modules" || name == "target" {
                    None
                } else {
                    Some(e)
                }
            })
            .collect();

        entries.sort_by_key(|e| e.path());

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                Self::hash_dir(&path, hasher)?;
            } else {
                let content = fs::read(&path)?;
                hasher.update(&content);
            }
        }

        Ok(())
    }

    /// Get package size in bytes
    pub fn size(&self) -> Result<u64> {
        let mut total = 0u64;

        fn count_size(path: &Path) -> Result<u64> {
            let mut total = 0u64;
            if path.is_dir() {
                for entry in fs::read_dir(path)? {
                    let entry = entry?;
                    let path = entry.path();
                    if path.is_file() {
                        total += fs::metadata(&path)?.len();
                    } else if path.is_dir() {
                        total += count_size(&path)?;
                    }
                }
            } else if path.is_file() {
                total = fs::metadata(path)?.len();
            }
            Ok(total)
        }

        total += count_size(&self.root)?;
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_manifest_validation() {
        let manifest = SkillManifest {
            name: "test-skill".to_string(),
            version: "1.0.0".to_string(),
            description: "Test skill".to_string(),
            tags: vec!["test".to_string()],
            capabilities: vec![],
            requirements: ResourceRequirements::default(),
            pricing: None,
            author: None,
            license: "MIT".to_string(),
            homepage: None,
            repository: None,
        };

        // Should fail - no capabilities
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn test_skill_id() {
        let manifest = SkillManifest {
            name: "csv-analyzer".to_string(),
            version: "1.0.0".to_string(),
            description: "Analyze CSV files".to_string(),
            tags: vec!["data".to_string()],
            capabilities: vec![CapabilityDetail {
                name: "analyze".to_string(),
                description: "Analyze data".to_string(),
                input_schema: "{}".to_string(),
                output_schema: "{}".to_string(),
                examples: vec![],
            }],
            requirements: ResourceRequirements::default(),
            pricing: None,
            author: Some("alice.near".to_string()),
            license: "MIT".to_string(),
            homepage: None,
            repository: None,
        };

        assert_eq!(manifest.skill_id(), "csv-analyzer@1.0.0");
    }
}
