use chrono::Utc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Community {
    pub id: String,
    pub label: String,
    pub chunk_ids: Vec<String>,
    pub created_at: String,
}

pub fn single_community(label: &str, chunk_ids: Vec<String>) -> Community {
    Community {
        id: uuid::Uuid::new_v4().to_string(),
        label: label.to_string(),
        chunk_ids,
        created_at: Utc::now().to_rfc3339(),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn placeholder_compiles() {
        assert_eq!(2 + 2, 4);
    }
}
