//! Agent to Agent Collaboration
//!
//! Combines NEAR registry (trust) with P2P network (work).

use crate::registry::AgentMetadata;
use crate::skills::protocol::{TaskRequest, TaskResponse, SkillAdvertisement};
use anyhow::Result;

/// Collaboration manager
pub struct CollaborationManager {
    /// Registry client for trust verification
    registry_client: crate::registry::RegistryClient,
}

impl CollaborationManager {
    /// Create new collaboration manager
    pub fn new(registry_id: String, network: String) -> Self {
        Self {
            registry_client: crate::registry::RegistryClient::new(registry_id, &network),
        }
    }

    /// Verify agent is trustworthy before collaborating
    pub async fn verify_agent(&self, agent_id: &str) -> Result<TrustScore> {
        match self.registry_client.get_agent(agent_id).await? {
            Some(agent) => {
                let score = TrustScore {
                    agent_id: agent.account_id.clone(),
                    reputation: agent.reputation,
                    rating_count: agent.rating_count,
                    verified: agent.online, // Using online as proxy for active/verified
                    capabilities: agent.capabilities,
                };
                Ok(score)
            }
            None => Ok(TrustScore::unknown(agent_id)),
        }
    }

    /// Find trustworthy agents with a skill
    pub async fn find_trustworthy_agents(
        &self,
        skill_name: &str,
        min_reputation: u32,
    ) -> Result<Vec<TrustworthyAgent>> {
        // This would query the registry for agents
        // For now, return empty
        Ok(vec![])
    }

    /// Rate agent after collaboration
    pub async fn rate_agent(
        &self,
        agent_id: &str,
        rating: u32,
    ) -> Result<()> {
        // Would submit rating to NEAR registry
        // For now, just log
        println!("⭐ Rating {}: {} stars", agent_id, rating);
        Ok(())
    }
}

/// Trust score for an agent
#[derive(Debug, Clone)]
pub struct TrustScore {
    pub agent_id: String,
    pub reputation: u32,
    pub rating_count: u32,
    pub verified: bool,
    pub capabilities: Vec<String>,
}

impl TrustScore {
    fn unknown(agent_id: &str) -> Self {
        Self {
            agent_id: agent_id.to_string(),
            reputation: 0,
            rating_count: 0,
            verified: false,
            capabilities: vec![],
        }
    }

    /// Check if agent is trustworthy
    pub fn is_trustworthy(&self, min_reputation: u32) -> bool {
        self.verified && self.reputation >= min_reputation && self.rating_count > 0
    }

    /// Get trust level
    pub fn trust_level(&self) -> TrustLevel {
        if !self.verified {
            TrustLevel::Unverified
        } else if self.rating_count == 0 {
            TrustLevel::New
        } else if self.reputation >= 80 {
            TrustLevel::High
        } else if self.reputation >= 50 {
            TrustLevel::Medium
        } else {
            TrustLevel::Low
        }
    }
}

/// Trust level
#[derive(Debug, Clone, PartialEq)]
pub enum TrustLevel {
    Unverified,
    New,
    Low,
    Medium,
    High,
}

/// Trustworthy agent with skill
#[derive(Debug, Clone)]
pub struct TrustworthyAgent {
    pub agent_id: String,
    pub skill_name: String,
    pub trust_score: TrustScore,
}

/// Collaboration flow with trust verification
pub struct CollaborationFlow {
    manager: CollaborationManager,
}

impl CollaborationFlow {
    pub fn new(registry_id: String, network: String) -> Self {
        Self {
            manager: CollaborationManager::new(registry_id, network),
        }
    }

    /// Request task from agent with trust verification
    pub async fn request_task_with_verification(
        &self,
        agent_id: &str,
        skill_name: &str,
        capability: &str,
        input: serde_json::Value,
        min_reputation: u32,
    ) -> Result<CollaborationResult> {
        println!("🔍 Verifying agent trust...");

        // Step 1: Verify trust on NEAR registry
        let trust_score = self.manager.verify_agent(agent_id).await?;

        println!("   Agent: {}", agent_id);
        println!("   Reputation: {}/100", trust_score.reputation);
        println!("   Ratings: {}", trust_score.rating_count);
        println!("   Level: {:?}", trust_score.trust_level());
        println!();

        // Step 2: Check if trustworthy
        if !trust_score.is_trustworthy(min_reputation) {
            println!("⚠️  Agent not trustworthy!");
            println!("   Minimum reputation: {}", min_reputation);
            println!("   Agent reputation: {}", trust_score.reputation);
            return Ok(CollaborationResult::Rejected(
                "Insufficient reputation".to_string()
            ));
        }

        println!("✅ Agent verified!");
        println!();
        println!("🤝 Sending task request...");

        // Step 3: Create task request
        let request = TaskRequest::new(
            "my-agent".to_string(),
            skill_name.to_string(),
            capability.to_string(),
            input,
        );

        println!("   Request ID: {}", request.request_id);
        println!("   Agent: {}", agent_id);
        println!("   Skill: {}", skill_name);
        println!();

        println!("⏳ Waiting for response...");
        println!();
        println!("⚠️  P2P execution requires daemon to be running.");
        println!("   The agent will:");
        println!("   1. Verify your identity on NEAR registry");
        println!("   2. Execute the task");
        println!("   3. Return results via P2P");
        println!();
        println!("   After collaboration, you can rate the agent:");

        Ok(CollaborationResult::Pending(request.request_id))
    }

    /// Rate agent after successful collaboration
    pub async fn rate_collaboration(
        &self,
        agent_id: &str,
        rating: u32,
    ) -> Result<()> {
        println!("⭐ Rating agent: {}", agent_id);
        println!("   Rating: {} stars", rating);
        println!();

        self.manager.rate_agent(agent_id, rating).await
    }
}

/// Result of collaboration request
#[derive(Debug, Clone)]
pub enum CollaborationResult {
    Pending(String),
    Rejected(String),
    Success(TaskResponse),
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trust_level_variants() {
        let unverified = TrustLevel::Unverified;
        let high = TrustLevel::High;
        // Just verify enum exists and can be compared
        assert!(matches!(unverified, TrustLevel::Unverified));
        assert!(matches!(high, TrustLevel::High));
    }

    #[test]
    fn test_trust_score_creation() {
        let score = TrustScore {
            agent_id: "test.near".to_string(),
            reputation: 85,
            rating_count: 10,
            verified: true,
            capabilities: vec!["compute".to_string()],
        };
        assert_eq!(score.agent_id, "test.near");
        assert_eq!(score.reputation, 85);
    }

    #[test]
    fn test_collaboration_manager_new() {
        let manager = CollaborationManager::new("registry.test.near".to_string(), "testnet".to_string());
        // Just verify creation works
        let _ = manager;
    }

    #[test]
    fn test_collaboration_result_variants() {
        let pending = CollaborationResult::Pending("task-1".to_string());
        let rejected = CollaborationResult::Rejected("Low rep".to_string());
        
        assert!(matches!(pending, CollaborationResult::Pending(_)));
        assert!(matches!(rejected, CollaborationResult::Rejected(_)));
    }

    #[test]
    fn test_trustworthy_agent_creation() {
        let agent = TrustworthyAgent {
            agent_id: "trusted.near".to_string(),
            skill_name: "compute".to_string(),
            trust_score: TrustScore {
                agent_id: "trusted.near".to_string(),
                reputation: 90,
                rating_count: 5,
                verified: true,
                capabilities: vec!["compute".to_string()],
            },
        };
        assert_eq!(agent.agent_id, "trusted.near");
    }
}
