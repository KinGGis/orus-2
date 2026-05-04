//! Simple test binary to verify Supabase connection
//!
//! Run with: cargo run -p wealthfolio-storage-postgres --bin test_connection
//!
//! Requires DATABASE_URL environment variable set to PostgreSQL connection string.

use diesel::prelude::*;
use diesel::sql_query;
use diesel::sql_types::Text;
use std::env;

#[derive(QueryableByName, Debug)]
struct Version {
    #[diesel(sql_type = Text)]
    version: String,
}

#[derive(QueryableByName, Debug)]
struct TableInfo {
    #[diesel(sql_type = Text)]
    table_name: String,
}

fn main() {
    println!("🔌 Wealthfolio PostgreSQL Connection Test");
    println!("==========================================\n");

    // Get database URL
    let database_url = match env::var("DATABASE_URL") {
        Ok(url) => {
            // Hide password in output
            let safe_url = url
                .split('@')
                .last()
                .map(|s| format!("postgresql://***@{}", s))
                .unwrap_or_else(|| "***hidden***".to_string());
            println!("📍 Connecting to: {}", safe_url);
            url
        }
        Err(_) => {
            eprintln!("❌ Error: DATABASE_URL environment variable not set");
            eprintln!("\n💡 Usage:");
            eprintln!("   $env:DATABASE_URL=\"postgresql://postgres.[ref]:[password]@aws-0-[region].pooler.supabase.com:6543/postgres\"");
            eprintln!("   cargo run -p wealthfolio-storage-postgres --bin test_connection");
            std::process::exit(1);
        }
    };

    // Test connection
    println!("\n📡 Testing connection...");
    
    let mut conn = match PgConnection::establish(&database_url) {
        Ok(c) => {
            println!("✅ Connection established successfully!");
            c
        }
        Err(e) => {
            eprintln!("❌ Failed to connect: {}", e);
            eprintln!("\n💡 Common issues:");
            eprintln!("   - Check your password is correct");
            eprintln!("   - Verify the project reference in the URL");
            eprintln!("   - Ensure your IP is allowed in Supabase dashboard");
            std::process::exit(1);
        }
    };

    // Get PostgreSQL version
    println!("\n📊 Database Info:");
    match sql_query("SELECT version() as version").get_result::<Version>(&mut conn) {
        Ok(v) => println!("   Version: {}", v.version),
        Err(e) => println!("   Could not get version: {}", e),
    }

    // List wf_ tables
    println!("\n📋 Wealthfolio Tables (wf_*):");
    let tables_query = sql_query(
        "SELECT table_name::text FROM information_schema.tables 
         WHERE table_schema = 'public' AND table_name LIKE 'wf_%' 
         ORDER BY table_name"
    );
    
    match tables_query.get_results::<TableInfo>(&mut conn) {
        Ok(tables) => {
            if tables.is_empty() {
                println!("   ⚠️  No wf_* tables found!");
                println!("   Run the migration first:");
                println!("   supabase db push");
            } else {
                println!("   Found {} tables:", tables.len());
                for table in &tables {
                    println!("   - {}", table.table_name);
                }
            }
        }
        Err(e) => println!("   Could not list tables: {}", e),
    }

    // Count rows in key tables
    println!("\n📈 Table Row Counts:");
    let count_queries = [
        ("wf_users", "SELECT COUNT(*)::text as version FROM wf_users"),
        ("wf_accounts", "SELECT COUNT(*)::text as version FROM wf_accounts"),
        ("wf_assets", "SELECT COUNT(*)::text as version FROM wf_assets"),
        ("wf_activities", "SELECT COUNT(*)::text as version FROM wf_activities"),
    ];

    for (table, query) in count_queries {
        match sql_query(query).get_result::<Version>(&mut conn) {
            Ok(count) => println!("   {}: {} rows", table, count.version),
            Err(_) => println!("   {}: (table may not exist)", table),
        }
    }

    println!("\n✨ Connection test complete!");
}
