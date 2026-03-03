//! Agent Collaboration Protocol
//!
//! Agents send task requests to each other and execute skills.

use serde::{Deserialize, Serialize};
use crate::skills::manifest::SkillManifest;

/// Protocol message types
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum AgentMessage {
    /// Skill advertisement
    SkillAdvertisement(SkillAdvertisement),

    /// Task request
    TaskRequest(TaskRequest),

    /// Task response
    TaskResponse(TaskResponse),

    /// Capability query
    CapabilityQuery(CapabilityQuery),

    /// Capability response
    CapabilityResponse(CapabilityResponse),
}

/// Skill advertisement message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillAdvertisement {
    /// Agent ID
    pub agent_id: String,

    /// Skill name
    pub skill_name: String,

    /// Version
    pub version: String,

    /// Description
    pub description: String,

    /// Tags for discovery
    pub tags: Vec<String>,

    /// Available capabilities
    pub capabilities: Vec<String>,

    /// Resource requirements
    pub requirements: SkillRequirements,

    /// Timestamp
    pub timestamp: u64,
}

/// Resource requirements for a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillRequirements {
    pub timeout_secs: u32,
    pub memory_mb: u32,
}

/// Task request from one agent to another
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    /// Unique request ID
    pub request_id: String,

    /// Requesting agent
    pub from_agent: String,

    /// Target skill
    pub skill_name: String,

    /// Capability to execute
    pub capability: String,

    /// Input data (JSON)
    pub input: serde_json::Value,

    /// Timeout in seconds
    pub timeout: u32,

    /// Timestamp
    pub timestamp: u64,
}

/// Task response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResponse {
    /// Request ID this responds to
    pub request_id: String,

    /// Responding agent
    pub from_agent: String,

    /// Success or failure
    pub success: bool,

    /// Result data (JSON)
    pub result: Option<serde_json::Value>,

    /// Error message if failed
    pub error: Option<String>,

    /// Execution time in seconds
    pub execution_time: f64,

    /// Timestamp
    pub timestamp: u64,
}

/// Query for capabilities
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityQuery {
    /// Querying agent
    pub from_agent: String,

    /// Search term
    pub query: Option<String>,

    /// Tag filter
    pub tag: Option<String>,

    /// Timestamp
    pub timestamp: u64,
}

/// Response to capability query
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    /// Responding agent
    pub from_agent: String,

    /// Available skills
    pub skills: Vec<AvailableSkill>,

    /// Timestamp
    pub timestamp: u64,
}

/// Available skill info
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AvailableSkill {
    pub name: String,
    pub version: String,
    pub description: String,
    pub tags: Vec<String>,
    pub capabilities: Vec<String>,
}

impl TaskRequest {
    /// Create new task request
    pub fn new(
        from_agent: String,
        skill_name: String,
        capability: String,
        input: serde_json::Value,
    ) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            from_agent,
            skill_name,
            capability,
            input,
            timeout: 30,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Create with timeout
    pub fn with_timeout(
        from_agent: String,
        skill_name: String,
        capability: String,
        input: serde_json::Value,
        timeout: u32,
    ) -> Self {
        Self {
            request_id: uuid::Uuid::new_v4().to_string(),
            from_agent,
            skill_name,
            capability,
            input,
            timeout,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl TaskResponse {
    /// Create success response
    pub fn success(request_id: String, from_agent: String, result: serde_json::Value, execution_time: f64) -> Self {
        Self {
            request_id,
            from_agent,
            success: true,
            result: Some(result),
            error: None,
            execution_time,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }

    /// Create error response
    pub fn error(request_id: String, from_agent: String, error: String) -> Self {
        Self {
            request_id,
            from_agent,
            success: false,
            result: None,
            error: Some(error),
            execution_time: 0.0,
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl From<SkillManifest> for SkillAdvertisement {
    fn from(manifest: SkillManifest) -> Self {
        Self {
            agent_id: manifest.author.clone().unwrap_or_default(),
            skill_name: manifest.name.clone(),
            version: manifest.version.clone(),
            description: manifest.description.clone(),
            tags: manifest.tags.clone(),
            capabilities: manifest.capabilities.iter().map(|c| c.name.clone()).collect(),
            requirements: SkillRequirements {
                timeout_secs: manifest.requirements.timeout_secs,
                memory_mb: manifest.requirements.memory_mb,
            },
            timestamp: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        }
    }
}

impl From<SkillManifest> for AvailableSkill {
    fn from(manifest: SkillManifest) -> Self {
        Self {
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            tags: manifest.tags,
            capabilities: manifest.capabilities.iter().map(|c| c.name.clone()).collect(),
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================


// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_request_creation() {
        let request = TaskRequest::with_timeout(
            "caller.near".to_string(),
            "compute".to_string(),
            "hash".to_string(),
            serde_json::json!({"data": "test"}),
            30,
        );
        assert!(!request.request_id.is_empty());
        assert_eq!(request.skill_name, "compute");
    }

    #[test]
    fn test_task_response_success() {
        let response = TaskResponse::success(
            "req-1".to_string(),
            "executor.near".to_string(),
            serde_json::json!({"result": 42}),
            1.23,
        );
        assert_eq!(response.request_id, "req-1");
        assert!(response.success);
    }

    #[test]
    fn test_task_response_error() {
        let response = TaskResponse::error(
            "req-1".to_string(),
            "executor.near".to_string(),
            "Execution failed".to_string(),
        );
        assert!(!response.success);
        assert!(response.error.is_some());
    }

    #[test]
    fn test_skill_advertisement_creation() {
        let ad = SkillAdvertisement {
            agent_id: "agent.near".to_string(),
            skill_name: "data-processing".to_string(),
            version: "1.0.0".to_string(),
            description: "Process data".to_string(),
            tags: vec!["data".to_string()],
            capabilities: vec!["parse".to_string()],
            requirements: SkillRequirements {
                timeout_secs: 30,
                memory_mb: 512,
            },
            timestamp: 0,
        };
        assert_eq!(ad.skill_name, "data-processing");
    }

    #[test]
    fn test_capability_query_creation() {
        let query = CapabilityQuery {
            from_agent: "agent.near".to_string(),
            query: Some("compute".to_string()),
            tag: Some("ml".to_string()),
            timestamp: 0,
        };
        assert_eq!(query.from_agent, "agent.near");
    }

    #[test]
    fn test_available_skill_creation() {
        let skill = AvailableSkill {
            name: "image-resize".to_string(),
            version: "1.0.0".to_string(),
            description: "Resize images".to_string(),
            tags: vec!["image".to_string()],
            capabilities: vec!["resize".to_string()],
        };
        assert_eq!(skill.name, "image-resize");
    }
}
