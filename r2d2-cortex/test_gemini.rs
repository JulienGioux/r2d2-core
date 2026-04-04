use reqwest::Client;
use serde_json::json;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Démarrage du test Gemini...");

    // Simuler le payload d'erreur
    let mut args = std::env::args();
    args.next();
    let api_key = args
        .next()
        .unwrap_or_else(|| std::env::var("GEMINI_API_KEY").unwrap_or_default());

    if api_key.is_empty() {
        println!("PAS DE CLES API");
        return Ok(());
    }

    let url = format!("https://generativelanguage.googleapis.com/v1beta/models/gemini-2.5-flash:generateContent?key={}", api_key);
    let client = Client::new();

    let json_body = json!({
      "contents": [
        {
          "role": "user",
          "parts": [
            { "text": "utilise le tool pour dire hello" }
          ]
        }
      ],
      "tools": [
        {
          "functionDeclarations": [
            {
              "name": "github_mcp_say_hello",
              "description": "Dit bonjour",
              "parameters": {
                "type": "object",
                "properties": {
                  "nom": { "type": "string" }
                }
              }
            }
          ]
        }
      ]
    });

    let res = client
        .post(&url)
        .header("Content-Type", "application/json")
        .json(&json_body)
        .send()
        .await?;
    let status = res.status();
    println!("Status: {}", status);
    let txt = res.text().await?;
    println!("Response: {}", txt);

    Ok(())
}
