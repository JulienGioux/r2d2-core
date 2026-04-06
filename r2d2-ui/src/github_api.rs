use reqwest::{header, Client};
use serde::Deserialize;

#[derive(Deserialize)]
struct GithubRepo {
    full_name: String,
}

pub async fn fetch_user_repos() -> anyhow::Result<Vec<String>> {
    let token = r2d2_cortex::security::vault::Vault::get_api_key("GITHUB_PERSONAL_ACCESS_TOKEN");
    
    let mut headers = header::HeaderMap::new();
    headers.insert(header::USER_AGENT, header::HeaderValue::from_static("R2D2-Workspace-Agent"));
    if let Some(t) = token {
        let auth_value = format!("Bearer {}", t);
        if let Ok(val) = header::HeaderValue::from_str(&auth_value) {
            headers.insert(header::AUTHORIZATION, val);
        }
    } else {
        // If there's no token, we can't fetch private repos. Let's return empty or allow public fetching.
        // Returning empty so that the UI can warn the user.
        return Ok(vec![]);
    }

    let client = Client::builder()
        .default_headers(headers)
        .build()?;

    let response = client
        .get("https://api.github.com/user/repos?per_page=100&sort=updated")
        .send()
        .await?;
        
    if !response.status().is_success() {
        return Err(anyhow::anyhow!("GitHub API Error: {}", response.status()));
    }

    let repos: Vec<GithubRepo> = response.json().await?;
    let mut names: Vec<String> = repos.into_iter().map(|r| r.full_name).collect();
    
    // Default system repos to prepend if needed
    if !names.contains(&"JulienGioux/r2d2-core".to_string()) {
        names.insert(0, "JulienGioux/r2d2-core".to_string());
    }

    Ok(names)
}
