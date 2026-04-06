use std::collections::HashMap;
fn main() {
    let mut roles = HashMap::new();
    roles.insert("mistral-large-latest".to_string(), "reasoning".to_string());
    roles.insert("gemini-2.5-pro".to_string(), "reasoning".to_string());
    
    let input_provider = "mistral-large-latest";
    let provider_key = if input_provider == "openai" { "reasoning" } else { input_provider };
    
    let resolved_provider = roles.iter()
        .find(|(_, r)| r.as_str() == provider_key)
        .map(|(m, _)| m.clone())
        .unwrap_or_else(|| input_provider.to_string());
        
    println!("Resolved: {}", resolved_provider);
}
