use serde_json::Value;

/// Méthode récursive pour extraire la chaîne HTML encodée dans la réponse RPC brute.
pub fn find_html(v: &Value) -> Option<String> {
    if let Some(s) = v.as_str() {
        let lower = s.to_lowercase();
        // Recherche des signatures d'un payload interactif valide
        if lower.contains("<!doctype html") || s.contains("data-app-data=") {
            return Some(s.to_string());
        }
    }
    if let Some(arr) = v.as_array() {
        for item in arr {
            if let Some(html) = find_html(item) {
                return Some(html);
            }
        }
    }
    None
}
