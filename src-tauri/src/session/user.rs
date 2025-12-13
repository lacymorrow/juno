// User Management for Multi-User Support

use serde::{Deserialize, Serialize};
use std::time::SystemTime;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct User {
    pub id: String,
    pub username: String,
    pub created_at: SystemTime,
    pub permissions: UserPermissions,
    pub quota: UserQuota,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserPermissions {
    pub role: UserRole,
    pub can_create_sessions: bool,
    pub can_manage_workspaces: bool,
    pub can_use_models: Vec<String>, // List of allowed models
    pub can_access_tools: Vec<String>, // List of allowed tools
    pub max_concurrent_sessions: usize,
    pub allowed_isolation_levels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum UserRole {
    Admin,
    Developer,
    User,
    Guest,
    Educational, // Special role for training/education
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserQuota {
    pub max_tokens_per_day: Option<usize>,
    pub max_storage_mb: Option<usize>,
    pub max_cpu_minutes_per_day: Option<usize>,
    pub max_sessions_per_day: Option<usize>,
    pub usage: UserUsage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserUsage {
    pub tokens_used_today: usize,
    pub storage_used_mb: usize,
    pub cpu_minutes_used_today: usize,
    pub sessions_created_today: usize,
    pub last_reset: SystemTime,
}

impl Default for UserPermissions {
    fn default() -> Self {
        Self {
            role: UserRole::User,
            can_create_sessions: true,
            can_manage_workspaces: true,
            can_use_models: vec!["anthropic/claude-3-5-sonnet".to_string()],
            can_access_tools: vec![],
            max_concurrent_sessions: 3,
            allowed_isolation_levels: vec!["basic".to_string()],
        }
    }
}

impl Default for UserQuota {
    fn default() -> Self {
        Self {
            max_tokens_per_day: Some(100000),
            max_storage_mb: Some(1024),
            max_cpu_minutes_per_day: Some(60),
            max_sessions_per_day: Some(10),
            usage: UserUsage::default(),
        }
    }
}

impl Default for UserUsage {
    fn default() -> Self {
        Self {
            tokens_used_today: 0,
            storage_used_mb: 0,
            cpu_minutes_used_today: 0,
            sessions_created_today: 0,
            last_reset: SystemTime::now(),
        }
    }
}

impl UserQuota {
    pub fn check_quota(&mut self, resource: &str, amount: usize) -> Result<(), String> {
        // Reset daily quotas if needed
        self.reset_if_needed();
        
        match resource {
            "tokens" => {
                if let Some(max) = self.max_tokens_per_day {
                    if self.usage.tokens_used_today + amount > max {
                        return Err("Token quota exceeded".to_string());
                    }
                }
                self.usage.tokens_used_today += amount;
            },
            "storage" => {
                if let Some(max) = self.max_storage_mb {
                    if self.usage.storage_used_mb + amount > max {
                        return Err("Storage quota exceeded".to_string());
                    }
                }
                self.usage.storage_used_mb += amount;
            },
            "cpu" => {
                if let Some(max) = self.max_cpu_minutes_per_day {
                    if self.usage.cpu_minutes_used_today + amount > max {
                        return Err("CPU quota exceeded".to_string());
                    }
                }
                self.usage.cpu_minutes_used_today += amount;
            },
            "sessions" => {
                if let Some(max) = self.max_sessions_per_day {
                    if self.usage.sessions_created_today + amount > max {
                        return Err("Session quota exceeded".to_string());
                    }
                }
                self.usage.sessions_created_today += amount;
            },
            _ => {}
        }
        
        Ok(())
    }
    
    fn reset_if_needed(&mut self) {
        if let Ok(elapsed) = self.usage.last_reset.elapsed() {
            if elapsed.as_secs() > 86400 { // 24 hours
                self.usage.tokens_used_today = 0;
                self.usage.cpu_minutes_used_today = 0;
                self.usage.sessions_created_today = 0;
                self.usage.last_reset = SystemTime::now();
            }
        }
    }
}