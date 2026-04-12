use serde_json::Value;
use crate::errors::{NotebookError, Result};

pub struct RpcDomain;

impl RpcDomain {
    /// Prépare le payload batchexecute attendu par Google
    /// Mimic de `build_request_body`
    pub fn build_batchexecute_body(rpc_id: &str, params: Value) -> Result<String> {
        let params_str = serde_json::to_string(&params)
            .map_err(|e| NotebookError::PayloadParsingError(e.to_string()))?;
        
        let inner = serde_json::json!([
            rpc_id,
            params_str,
            serde_json::Value::Null,
            "generic"
        ]);
        
        let freq = serde_json::json!([[inner]]);
        
        serde_json::to_string(&freq)
            .map_err(|e| NotebookError::PayloadParsingError(e.to_string()))
    }
}
