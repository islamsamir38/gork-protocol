//! Agent Skills module
//!
//! P2P skill sharing and agent collaboration on the Gork network.

pub mod manifest;
pub mod protocol;
pub mod collaboration;

pub use manifest::{SkillManifest, SkillPackage};
pub use protocol::{
    AgentMessage, SkillAdvertisement, TaskRequest, TaskResponse,
    CapabilityQuery, CapabilityResponse, AvailableSkill
};
pub use collaboration::{
    CollaborationManager, CollaborationFlow, TrustScore, TrustLevel, CollaborationResult
};

use anyhow::Result;
use std::path::{Path, PathBuf};

/// Skills module error type
#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    #[error("Manifest not found: {0}")]
    ManifestNotFound(String),

    #[error("Invalid manifest: {0}")]
    InvalidManifest(String),

    #[error("Skill package error: {0}")]
    PackageError(String),
}

/// Agent skills directory
pub fn skills_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".gork-agent").join("skills")
}

/// Install a skill locally (not on-chain)
pub fn install_skill(package_path: &Path) -> Result<SkillManifest> {
    // Create skills directory
    let skills_dir = skills_dir();
    std::fs::create_dir_all(&skills_dir)?;

    // Load skill package
    let package = SkillPackage::load(package_path)?;
    package.manifest.validate()?;

    // Create skill directory
    let skill_dir = skills_dir.join(&package.manifest.name);
    std::fs::create_dir_all(&skill_dir)?;

    // Copy skill.yaml
    let dest = skill_dir.join("skill.yaml");
    std::fs::copy(package_path.join("skill.yaml"), &dest)?;

    // Copy code directory if exists
    let code_src = package_path.join("code");
    if code_src.exists() {
        let code_dst = skill_dir.join("code");
        copy_dir_recursive(&code_src, &code_dst)?;
    }

    println!("✅ Skill installed locally: {}", package.manifest.name);
    println!("   Location: {}", skill_dir.display());

    Ok(package.manifest)
}

/// List locally installed skills
pub fn list_local_skills() -> Result<Vec<SkillManifest>> {
    let skills_dir = skills_dir();
    let mut skills = Vec::new();

    if !skills_dir.exists() {
        return Ok(skills);
    }

    for entry in std::fs::read_dir(skills_dir)? {
        let entry = entry?;
        let skill_path = entry.path();

        // Skip if not directory
        if !skill_path.is_dir() {
            continue;
        }

        // Load skill.yaml
        let manifest_path = skill_path.join("skill.yaml");
        if manifest_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            if let Ok(manifest) = serde_yaml::from_str::<SkillManifest>(&content) {
                skills.push(manifest);
            }
        }
    }

    Ok(skills)
}

/// Get local skill by name
pub fn get_local_skill(name: &str) -> Result<Option<SkillManifest>> {
    let skills = list_local_skills()?;
    Ok(skills.into_iter().find(|s| s.name == name))
}

/// Remove a local skill
pub fn remove_local_skill(name: &str) -> Result<bool> {
    let skills_dir = skills_dir();
    let skill_dir = skills_dir.join(name);

    if skill_dir.exists() {
        std::fs::remove_dir_all(&skill_dir)?;
        println!("🗑️  Skill removed: {}", name);
        Ok(true)
    } else {
        Ok(false)
    }
}

/// Copy directory recursively
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;

    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}
