#[tokio::main]
async fn main() {
    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgres://r2d2_admin:secure_r2d2_password_local@localhost:5433/r2d2_blackboard".to_string());
    println!("Connecting to {}", db_url);
    let blackboard = r2d2_blackboard::PostgresBlackboard::new(&db_url).await.unwrap();
    match blackboard.get_all_mcp_tools().await {
        Ok(t) => println!("Tools: {}", t.len()),
        Err(e) => println!("Error: {:?}", e),
    }
}
