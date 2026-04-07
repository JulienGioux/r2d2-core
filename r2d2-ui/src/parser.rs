use serde_json::Value;

/// Extracts underlying text content from a potentially nested/hallucinated JSON payload.
/// Models sometimes recursively nest Paradox Engine payloads instead of rendering text directly.
pub fn extract_markdown(json_resp: &str) -> (String, String, String, String) {
    let mut current_json = json_resp.to_string();
    let mut final_content = current_json.clone();

    // Fallback meta values
    let mut model_name = "Paradox Local".to_string();
    let mut consensus = "Unknown".to_string();
    let mut latency = "paradox-multiapi-0".to_string();

    let mut depth = 0;
    while let Ok(parsed) = serde_json::from_str::<Value>(&current_json) {
        if depth == 0 {
            // Unpack main envelope attributes
            if let Some(source) = parsed
                .get("source")
                .and_then(|s| s.get("ParadoxEngine"))
                .and_then(|s| s.as_str())
            {
                model_name = source.to_string();
            }
            if let Some(c) = parsed.get("consensus").and_then(|c| c.as_str()) {
                consensus = c.to_string();
            }
            if let Some(id) = parsed.get("id").and_then(|id| id.as_str()) {
                latency = id.replace("paradox-multiapi-", "");
            }
        }

        if let Some(content_val) = parsed.get("content") {
            if let Some(content_str) = content_val.as_str() {
                // If it looks like another JSON wrapper inside, loop again
                if content_str.trim().starts_with('{') {
                    current_json = content_str.to_string();
                    depth += 1;
                    continue;
                } else {
                    final_content = content_str.to_string();
                    break;
                }
            } else {
                final_content = content_val.to_string();
                break;
            }
        } else {
            break; // No content field, stop unpacking
        }
    }

    (final_content, model_name, consensus, latency)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_json() {
        let payload = r#"{
            "id": "paradox-multiapi-1234",
            "source": { "ParadoxEngine": "Mistral" },
            "consensus": "UniversalSynthesis",
            "content": "Clean markdown response."
        }"#;

        let (content, model, consensus, latency) = extract_markdown(payload);
        assert_eq!(content, "Clean markdown response.");
        assert_eq!(model, "Mistral");
        assert_eq!(consensus, "UniversalSynthesis");
        assert_eq!(latency, "1234");
    }

    #[test]
    fn test_hallucinated_nested_json() {
        // TDD case: Gemini hallucinates formatting inside the content payload.
        let payload = r#"{
            "id": "paradox-multiapi-9999",
            "source": { "ParadoxEngine": "Gemini Cloud" },
            "consensus": "CloudDistillation",
            "content": "{\"id\": \"paradox-multiapi-1111\", \"source\": { \"ParadoxEngine\": \"Gemini Cloud\" }, \"content\": \"Ceci est le vrai message sans JSON !\"}"
        }"#;

        let (content, model, consensus, latency) = extract_markdown(payload);
        assert_eq!(content, "Ceci est le vrai message sans JSON !");
        // Attributes should be captured from the first valid JSON envelope.
        assert_eq!(model, "Gemini Cloud");
        assert_eq!(consensus, "CloudDistillation");
        assert_eq!(latency, "9999");
    }

    #[test]
    fn test_raw_string_fallback() {
        let payload = "Just some text, not JSON.";
        let (content, _, _, _) = extract_markdown(payload);
        assert_eq!(content, "Just some text, not JSON.");
    }
}
