//! Human-authored implementation plans, repository group import and document export.
mod repository;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

pub fn new_id() -> String {
    Uuid::new_v4().simple().to_string()
}
fn version() -> u64 {
    1
}
fn schema() -> String {
    "0.3".into()
}
fn function() -> String {
    "function".into()
}
fn new_kind() -> String {
    "new".into()
}
fn uses_kind() -> String {
    "uses".into()
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Plan {
    pub schema_version: String,
    pub version: u64,
    pub summary: String,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub requests: Vec<serde_json::Value>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Plan {
    fn default() -> Self {
        Self {
            schema_version: schema(),
            version: version(),
            summary: String::new(),
            nodes: vec![],
            edges: vec![],
            requests: vec![],
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Node {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub path: String,
    pub description: String,
    pub files: Vec<String>,
    pub interfaces: Vec<Interface>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Node {
    fn default() -> Self {
        Self {
            id: new_id(),
            name: "새 모듈".into(),
            kind: new_kind(),
            path: "features/new-module/model".into(),
            description: String::new(),
            files: vec![],
            interfaces: vec![],
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Interface {
    pub name: String,
    pub kind: String,
    pub condition: String,
    pub result: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Interface {
    fn default() -> Self {
        Self {
            name: String::new(),
            kind: function(),
            condition: String::new(),
            result: String::new(),
            inputs: vec![],
            outputs: vec![],
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Edge {
    pub id: String,
    pub from: String,
    pub to: String,
    #[serde(rename = "type")]
    pub kind: String,
    pub uses: Vec<String>,
    #[serde(flatten)]
    pub extra: BTreeMap<String, serde_json::Value>,
}
impl Default for Edge {
    fn default() -> Self {
        Self {
            id: new_id(),
            from: String::new(),
            to: String::new(),
            kind: uses_kind(),
            uses: vec![],
            extra: BTreeMap::new(),
        }
    }
}

fn clean_relative(path: &str) -> bool {
    !path.is_empty()
        && !path.contains(['\\', ':'])
        && path.split('/').all(|part| !matches!(part, "" | "." | ".."))
}

impl Plan {
    /// Accept the old Tauri JSON, including its optional export wrapper. Reject malformed data
    /// instead of silently dropping modules or relationships during migration.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let document = value.get("plan").unwrap_or(&value);
        if !document["nodes"].is_array() || !document["edges"].is_array() {
            return Err("계획 JSON에는 nodes와 edges 배열이 필요합니다.".into());
        }
        let plan: Self = serde_json::from_value(document.clone()).map_err(|e| e.to_string())?;
        let errors = plan.errors();
        if errors.is_empty() {
            Ok(plan)
        } else {
            Err(errors.join("\n"))
        }
    }
    pub fn errors(&self) -> Vec<String> {
        let mut errors = vec![];
        let mut ids = HashSet::new();
        if self.nodes.len() > 1000 || self.edges.len() > 5000 {
            errors.push("모듈 1,000개 / 관계 5,000개를 초과했습니다.".into());
        }
        for node in &self.nodes {
            if node.id.is_empty() || !ids.insert(node.id.as_str()) {
                errors.push(format!("중복되거나 비어 있는 모듈 ID: {}", node.id));
            }
            let parts: Vec<_> = node.path.split('/').collect();
            let valid_path = match parts.first().copied() {
                Some("app" | "shared") => parts.len() >= 2,
                Some("pages" | "widgets" | "features" | "entities") => parts.len() >= 3,
                _ => false,
            };
            if !clean_relative(&node.path) || !valid_path {
                errors.push(format!(
                    "{}: Layer/Slice/Segment 경로를 확인하세요.",
                    node.name
                ));
            }
            if !matches!(node.kind.as_str(), "new" | "modify" | "existing") {
                errors.push(format!("{}: 알 수 없는 모듈 종류입니다.", node.name));
            }
            if node.files.iter().any(|file| !clean_relative(file)) {
                errors.push(format!(
                    "{}: 파일은 모듈 경로 기준의 상대 경로여야 합니다.",
                    node.name
                ));
            }
            if node
                .interfaces
                .iter()
                .any(|item| item.name.trim().is_empty())
            {
                errors.push(format!("{}: 인터페이스 이름을 입력하세요.", node.name));
            }
        }
        let mut edge_ids = HashSet::new();
        for edge in &self.edges {
            if !edge_ids.insert(edge.id.as_str()) || edge.id.is_empty() {
                errors.push("관계 ID가 중복되거나 비어 있습니다.".into());
            }
            if !ids.contains(edge.from.as_str()) || !ids.contains(edge.to.as_str()) {
                errors.push("존재하지 않는 모듈을 연결한 관계가 있습니다.".into());
            }
            if edge.from == edge.to {
                errors.push("모듈이 자기 자신에 연결되어 있습니다.".into());
            }
        }
        errors
    }
    pub fn json(&self) -> Result<String, String> {
        let errors = self.errors();
        if !errors.is_empty() {
            return Err(errors.join("\n"));
        }
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }
    pub fn markdown(&self) -> String {
        let mut out = format!(
            "# 구현 계획\n\n{}\n\n계획 버전: {}\n\n",
            self.summary, self.version
        );
        for node in &self.nodes {
            out.push_str(&format!(
                "## {} ({})\n\n- 경로: `{}`\n- 변경: {}\n- 책임: {}\n",
                node.name, node.id, node.path, node.kind, node.description
            ));
            for file in &node.files {
                out.push_str(&format!("- 파일: `{}/{file}`\n", node.path));
            }
            for item in &node.interfaces {
                out.push_str(&format!(
                    "\n### {} · {}\n\n- 조건: {}\n- 결과: {}\n- 입력: {}\n- 출력: {}\n",
                    item.name,
                    item.kind,
                    item.condition,
                    item.result,
                    item.inputs.join("; "),
                    item.outputs.join("; ")
                ));
            }
            out.push('\n');
        }
        if !self.edges.is_empty() {
            out.push_str("## 의존 관계\n\n");
        }
        for edge in &self.edges {
            let name = |id: &str| {
                self.nodes
                    .iter()
                    .find(|node| node.id == id)
                    .map(|node| node.name.as_str())
                    .unwrap_or("?")
            };
            out.push_str(&format!(
                "- {} ({}) → {} ({}): {} [{}]\n",
                name(&edge.from),
                edge.from,
                name(&edge.to),
                edge.to,
                edge.kind,
                edge.uses.join(", ")
            ));
        }
        if !self.requests.is_empty() {
            out.push_str("\n## 기록한 요청\n\n");
            for request in &self.requests {
                out.push_str(&format!(
                    "- {}\n",
                    request["text"].as_str().unwrap_or_default()
                ));
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn old_client_export_roundtrips_contracts_and_unknown_metadata() {
        let text = r#"{"schemaVersion":"0.3","plan":{"version":4,"nodes":[{"id":"a","name":"Cart","kind":"modify","path":"features/cart/model","description":"장바구니를 변경한다","files":["cart.ts"],"interfaces":[{"name":"add","kind":"function","inputs":["id: string"],"outputs":["void"],"condition":"추가할 때","result":"상품이 추가된다"}],"custom":"preserved"}],"edges":[],"requests":[]}}"#;
        let plan = Plan::from_json(text).unwrap();
        assert_eq!(Plan::from_json(&plan.json().unwrap()).unwrap(), plan);
        assert_eq!(plan.nodes[0].extra["custom"], "preserved");
        assert!(plan.markdown().contains("id: string"));
        assert!(plan.markdown().contains("features/cart/model/cart.ts"));
    }
    #[test]
    fn rejects_traversal_duplicate_ids_and_dangling_edges() {
        let mut plan = Plan::default();
        let node = Node {
            files: vec!["../escape.ts".into()],
            ..Default::default()
        };
        plan.nodes = vec![node.clone(), node];
        plan.edges.push(Edge {
            from: "missing".into(),
            to: "missing".into(),
            ..Default::default()
        });
        assert!(plan.errors().len() >= 4);
        assert!(plan.json().is_err());
        assert!(Plan::from_json("{}").is_err());
    }
}
