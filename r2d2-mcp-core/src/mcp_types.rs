use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Une Requête MCP formelle (JSON-RPC 2.0)
#[derive(Debug, Deserialize, Clone)]
#[serde(untagged)]
pub enum McpMessage {
    Request(McpRequest),
    Response(McpResponse),
}

#[derive(Debug, Deserialize, Clone)]
pub struct McpRequest {
    pub jsonrpc: String,
    pub id: Option<Value>, // l'id peut être un number, une string ou null
    pub method: String,
    pub params: Option<Value>,
}

/// Une Réponse MCP formelle (JSON-RPC 2.0)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<McpError>,
}

impl McpResponse {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Value, code: i32, message: &str) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(McpError {
                code,
                message: message.to_string(),
            }),
        }
    }
}

/// Structure d'Erreur MCP (JSON-RPC)
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct McpError {
    pub code: i32,
    pub message: String,
}

// --------------------------------------------------------
// ZERO-COPY IPC PAYLOADS (RKYV)
// "Charge Opaque Pattern"
// --------------------------------------------------------

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
#[archive(check_bytes)] // Validation necessaire car IPC
pub struct IpcMcpRequest {
    pub method: String,
    // La struct interne / JSON est gardee "opaque" sous forme d'octets.
    pub params_raw: Vec<u8>,
}

#[derive(rkyv::Archive, rkyv::Serialize, rkyv::Deserialize, Debug)]
#[archive(check_bytes)]
pub struct IpcMcpResponse {
    pub success: bool,
    pub result_raw: Vec<u8>, // JSON serialize
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_valid_request() {
        let raw = r#"{"jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{"name":"ask_notebook", "arguments":{"prompt":"Hello"}}}"#;
        let parsed: McpRequest = serde_json::from_str(raw).expect("Must parse valid JSONRPC");
        assert_eq!(parsed.method, "tools/call");
        assert!(parsed.params.is_some());
    }

    #[test]
    fn test_invalid_request() {
        let raw = r#"{"jsonrpc":"2.0", "params":{}}"#; // Missing method
        let parsed: Result<McpRequest, _> = serde_json::from_str(raw);
        assert!(
            parsed.is_err(),
            "Les requêtes non conformes doivent être rejetées mathématiquement par Serde"
        );
    }
}
