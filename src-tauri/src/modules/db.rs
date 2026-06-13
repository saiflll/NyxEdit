use serde::{Deserialize, Serialize};
use sqlx::Column;
use sqlx::Row;
use std::collections::HashMap;
use std::sync::Mutex;

// ── Types ─────────────────────────────────────────────────────────────────────

#[derive(Clone, Serialize, Deserialize, Debug)]
#[serde(rename_all = "lowercase")]
pub enum DbType {
    Postgres,
    Mysql,
    Sqlite,
    Mongodb,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DbConnectionInfo {
    pub id: String,
    pub db_type: DbType,
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: Option<String>,
    pub database: Option<String>,
    pub label: String,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct ColumnInfo {
    pub name: String,
    pub data_type: String,
    pub nullable: bool,
    pub is_pk: bool,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct QueryResult {
    pub columns: Vec<String>,
    pub rows: Vec<Vec<serde_json::Value>>,
    pub row_count: u64,
    pub duration_ms: u64,
}

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct DbTableInfo {
    pub name: String,
    pub schema: Option<String>,
}

// ── Connection pool enum ──────────────────────────────────────────────────────

enum DbPool {
    Pg(sqlx::PgPool),
    My(sqlx::MySqlPool),
    Sq(sqlx::SqlitePool),
    Mongo(mongodb::Client),
    #[allow(dead_code)]
    None,
}

pub struct DbEntry {
    pool: DbPool,
    info: DbConnectionInfo,
}

pub struct DbManager {
    connections: Mutex<HashMap<String, DbEntry>>,
}

impl DbManager {
    pub fn new() -> Self {
        Self {
            connections: Mutex::new(HashMap::new()),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn format_duration(start: std::time::Instant) -> u64 {
    start.elapsed().as_millis() as u64
}

fn build_conn_string(info: &DbConnectionInfo) -> String {
    match info.db_type {
        DbType::Postgres => {
            let pass = info.password.as_deref().unwrap_or("");
            format!(
                "postgresql://{}:{}@{}:{}/{}",
                info.user,
                pass,
                info.host,
                info.port,
                info.database.as_deref().unwrap_or("postgres")
            )
        }
        DbType::Mysql => {
            let pass = info.password.as_deref().unwrap_or("");
            format!(
                "mysql://{}:{}@{}:{}/{}",
                info.user,
                pass,
                info.host,
                info.port,
                info.database.as_deref().unwrap_or("mysql")
            )
        }
        DbType::Sqlite => {
            info.database.clone().unwrap_or_else(|| ":memory:".to_string())
        }
        DbType::Mongodb => {
            let pass = info.password.as_deref().unwrap_or("");
            let db = info.database.as_deref().unwrap_or("admin");
            format!(
                "mongodb://{}:{}@{}:{}/{}",
                info.user, pass, info.host, info.port, db
            )
        }
    }
}

// ── Tauri Commands ────────────────────────────────────────────────────────────

#[tauri::command]
pub async fn db_connect(
    state: tauri::State<'_, DbManager>,
    db_type: String,
    host: String,
    port: u16,
    user: String,
    password: Option<String>,
    database: Option<String>,
    label: Option<String>,
) -> Result<DbConnectionInfo, String> {
    let db_type_enum = match db_type.to_lowercase().as_str() {
        "postgres" | "postgresql" => DbType::Postgres,
        "mysql" | "mariadb" => DbType::Mysql,
        "sqlite" => DbType::Sqlite,
        "mongodb" | "mongo" => DbType::Mongodb,
        _ => return Err(format!("Unsupported database type: {}", db_type)),
    };

    let info = DbConnectionInfo {
        id: uuid::Uuid::new_v4().to_string(),
        db_type: db_type_enum.clone(),
        host: host.clone(),
        port,
        user: user.clone(),
        password: password.clone(),
        database: database.clone(),
        label: label.unwrap_or_else(|| format!("{}-{}", db_type, host)),
    };

    let conn_str = build_conn_string(&info);

    let pool = match db_type_enum {
        DbType::Postgres => {
            let p = sqlx::PgPool::connect(&conn_str)
                .await
                .map_err(|e| format!("PostgreSQL connect failed: {}", e))?;
            // Ping
            sqlx::query("SELECT 1").execute(&p).await.map_err(|e| format!("Ping failed: {}", e))?;
            DbPool::Pg(p)
        }
        DbType::Mysql => {
            let p = sqlx::MySqlPool::connect(&conn_str)
                .await
                .map_err(|e| format!("MySQL connect failed: {}", e))?;
            sqlx::query("SELECT 1").execute(&p).await.map_err(|e| format!("Ping failed: {}", e))?;
            DbPool::My(p)
        }
        DbType::Sqlite => {
            let p = sqlx::SqlitePool::connect(&conn_str)
                .await
                .map_err(|e| format!("SQLite connect failed: {}", e))?;
            sqlx::query("SELECT 1").execute(&p).await.map_err(|e| format!("Ping failed: {}", e))?;
            DbPool::Sq(p)
        }
        DbType::Mongodb => {
            let client = mongodb::Client::with_uri_str(&conn_str)
                .await
                .map_err(|e| format!("MongoDB connect failed: {}", e))?;
            // Ping
            let db = client.database("admin");
            db.run_command(mongodb::bson::doc! { "ping": 1 }, None)
                .await
                .map_err(|e| format!("MongoDB ping failed: {}", e))?;
            DbPool::Mongo(client)
        }
    };

    let entry = DbEntry {
        pool,
        info: info.clone(),
    };

    let mut connections = state.connections.lock().unwrap();
    connections.insert(info.id.clone(), entry);

    Ok(info)
}

#[tauri::command]
pub fn db_disconnect(
    state: tauri::State<'_, DbManager>,
    connection_id: String,
) -> Result<(), String> {
    let mut connections = state.connections.lock().unwrap();
    connections.remove(&connection_id);
    Ok(())
}

#[tauri::command]
pub fn db_list_connections(
    state: tauri::State<'_, DbManager>,
) -> Vec<DbConnectionInfo> {
    let connections = state.connections.lock().unwrap();
    connections.values().map(|e| e.info.clone()).collect()
}

#[tauri::command]
pub async fn db_query(
    state: tauri::State<'_, DbManager>,
    connection_id: String,
    sql: String,
) -> Result<QueryResult, String> {
    let start = std::time::Instant::now();

    let (pool, mongo_info) = {
        let connections = state.connections.lock().unwrap();
        let entry = connections.get(&connection_id).ok_or("Connection not found")?;
        let info = entry.info.clone();
        let pool = match &entry.pool {
            DbPool::Pg(p) => DbPool::Pg(p.clone()),
            DbPool::My(p) => DbPool::My(p.clone()),
            DbPool::Sq(p) => DbPool::Sq(p.clone()),
            DbPool::Mongo(c) => DbPool::Mongo(c.clone()),
            DbPool::None => return Err("Not connected".to_string()),
        };
        (pool, info)
    };

    if let DbPool::Mongo(client) = pool {
        return exec_mongo_query(client, &mongo_info, &sql, start).await;
    }

    macro_rules! do_query {
        ($pool:expr) => {{
            let rows = sqlx::query(&sql).fetch_all($pool).await.map_err(|e| format!("Query error: {}", e))?;
            if rows.is_empty() {
                (vec![], vec![], 0u64)
            } else {
                let cols: Vec<String> = rows[0].columns().iter().map(|c| c.name().to_string()).collect();
                let mut result_rows = Vec::new();
                for row in &rows {
                    let mut vals = Vec::new();
                    for col in &cols {
                        let val = row.try_get::<String, _>(col.as_str())
                            .ok()
                            .map(cell_str_to_json)
                            .unwrap_or(serde_json::Value::Null);
                        vals.push(val);
                    }
                    result_rows.push(vals);
                }
                (cols, result_rows, rows.len() as u64)
            }
        }};
    }

    match pool {
        DbPool::Pg(p) => {
            let (columns, rows, row_count) = do_query!(&p);
            Ok(QueryResult { columns, rows, row_count, duration_ms: format_duration(start) })
        }
        DbPool::My(p) => {
            let (columns, rows, row_count) = do_query!(&p);
            Ok(QueryResult { columns, rows, row_count, duration_ms: format_duration(start) })
        }
        DbPool::Sq(p) => {
            let (columns, rows, row_count) = do_query!(&p);
            Ok(QueryResult { columns, rows, row_count, duration_ms: format_duration(start) })
        }
        _ => Err("Not connected".to_string()),
    }
}

fn cell_str_to_json(s: String) -> serde_json::Value {
    if s.is_empty() { return serde_json::Value::Null; }
    if let Ok(n) = s.parse::<i64>() { return serde_json::Value::Number(n.into()); }
    if let Ok(n) = s.parse::<f64>() {
        if let Some(num) = serde_json::Number::from_f64(n) {
            return serde_json::Value::Number(num);
        }
    }
    if s == "true" || s == "t" || s == "1" { return serde_json::Value::Bool(true); }
    if s == "false" || s == "f" || s == "0" { return serde_json::Value::Bool(false); }
    serde_json::Value::String(s)
}

async fn exec_mongo_query(
    client: mongodb::Client,
    info: &DbConnectionInfo,
    sql: &str,
    start: std::time::Instant,
) -> Result<QueryResult, String> {
    let parts: Vec<&str> = sql.trim().splitn(3, ' ').collect();
    if parts.len() < 2 {
        return Err("MongoDB query format: find|aggregate <collection> [json]".to_string());
    }
    let command = parts[0].to_lowercase();
    let collection_name = parts[1];
    let filter_str = parts.get(2).copied().unwrap_or("{}");

    let db_name = info.database.as_deref().unwrap_or("admin");
    let db = client.database(db_name);
    let collection = db.collection::<mongodb::bson::Document>(collection_name);

    match command.as_str() {
        "find" | "list" => {
            let filter: mongodb::bson::Document = serde_json::from_str(filter_str)
                .map_err(|e| format!("Invalid filter JSON: {}", e))?;
            let mut cursor = collection
                .find(filter, None)
                .await
                .map_err(|e| format!("MongoDB find failed: {}", e))?;

            let mut rows = Vec::new();
            let mut columns: Vec<String> = Vec::new();
            use futures::TryStreamExt;
            while let Some(doc) = cursor.try_next().await.map_err(|e| format!("Cursor error: {}", e))? {
                let val: serde_json::Value =
                    mongodb::bson::from_bson(mongodb::bson::Bson::Document(doc))
                        .unwrap_or(serde_json::Value::Null);
                if let serde_json::Value::Object(map) = &val {
                    if columns.is_empty() {
                        columns = map.keys().cloned().collect();
                    }
                    let vals: Vec<serde_json::Value> = columns.iter().map(|k| map.get(k).cloned().unwrap_or(serde_json::Value::Null)).collect();
                    rows.push(vals);
                }
            }
            let row_count = rows.len() as u64;
            Ok(QueryResult {
                columns,
                rows,
                row_count,
                duration_ms: format_duration(start),
            })
        }
        _ => Err(format!("Unknown MongoDB command: {}", command)),
    }
}

#[tauri::command]
pub async fn db_list_databases(
    state: tauri::State<'_, DbManager>,
    connection_id: String,
) -> Result<Vec<String>, String> {
    let pool_clone = {
        let connections = state.connections.lock().unwrap();
        let entry = connections.get(&connection_id).ok_or("Connection not found")?;
        match &entry.pool {
            DbPool::Pg(p) => DbPool::Pg(p.clone()),
            DbPool::My(p) => DbPool::My(p.clone()),
            DbPool::Sq(p) => DbPool::Sq(p.clone()),
            DbPool::Mongo(c) => DbPool::Mongo(c.clone()),
            DbPool::None => return Err("Not connected".to_string()),
        }
    };

    match pool_clone {
        DbPool::Pg(p) => {
            let rows = sqlx::query("SELECT datname FROM pg_database WHERE datistemplate = false ORDER BY datname")
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            let dbs: Vec<String> = rows.iter().map(|r| {
                let name: String = r.get(0);
                name
            }).collect();
            Ok(dbs)
        }
        DbPool::My(p) => {
            let rows = sqlx::query("SHOW DATABASES")
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            let dbs: Vec<String> = rows.iter().map(|r| {
                let name: String = r.get::<String, _>(0);
                name
            }).collect();
            Ok(dbs)
        }
        DbPool::Sq(_) => {
            // SQLite doesn't have multiple databases in the same sense
            Ok(vec!["main".to_string()])
        }
        DbPool::Mongo(client) => {
            let db_names = client
                .list_database_names(None, None)
                .await
                .map_err(|e| format!("MongoDB list databases: {}", e))?;
            Ok(db_names)
        }
        DbPool::None => Err("Not connected".to_string()),
    }
}

#[tauri::command]
pub async fn db_list_tables(
    state: tauri::State<'_, DbManager>,
    connection_id: String,
    database: Option<String>,
) -> Result<Vec<DbTableInfo>, String> {
    let (pool_clone, db_info) = {
        let connections = state.connections.lock().unwrap();
        let entry = connections.get(&connection_id).ok_or("Connection not found")?;
        let db = entry.info.database.clone();
        let pool = match &entry.pool {
            DbPool::Pg(p) => DbPool::Pg(p.clone()),
            DbPool::My(p) => DbPool::My(p.clone()),
            DbPool::Sq(p) => DbPool::Sq(p.clone()),
            DbPool::Mongo(c) => DbPool::Mongo(c.clone()),
            DbPool::None => return Err("Not connected".to_string()),
        };
        (pool, db)
    };

    match pool_clone {
        DbPool::Pg(p) => {
            let schema = database.unwrap_or_else(|| "public".to_string());
            let rows = sqlx::query(
                "SELECT table_name, table_schema FROM information_schema.tables WHERE table_schema = $1 AND table_type = 'BASE TABLE' ORDER BY table_name"
            )
                .bind(&schema)
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            let tables: Vec<DbTableInfo> = rows.iter().map(|r| {
                let name: String = r.get::<String, _>(0);
                let schema: String = r.get::<String, _>(1);
                DbTableInfo { name, schema: Some(schema) }
            }).collect();
            Ok(tables)
        }
        DbPool::My(p) => {
            let db_name = database.or(db_info).unwrap_or_else(|| "mysql".to_string());
            let rows = sqlx::query("SELECT TABLE_NAME, TABLE_SCHEMA FROM information_schema.tables WHERE table_schema = ? AND table_type = 'BASE TABLE' ORDER BY TABLE_NAME")
                .bind(&db_name)
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            let tables: Vec<DbTableInfo> = rows.iter().map(|r| {
                let name: String = r.get::<String, _>(0);
                let schema: String = r.get::<String, _>(1);
                DbTableInfo { name, schema: Some(schema) }
            }).collect();
            Ok(tables)
        }
        DbPool::Sq(p) => {
            let rows = sqlx::query("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            let tables: Vec<DbTableInfo> = rows.iter().map(|r| {
                let name: String = r.get::<String, _>(0);
                DbTableInfo { name, schema: None }
            }).collect();
            Ok(tables)
        }
        DbPool::Mongo(client) => {
            let db_name = database.or(db_info).unwrap_or_else(|| "admin".to_string());
            let db = client.database(&db_name);
            let mut coll_names = db
                .list_collection_names(None)
                .await
                .map_err(|e| format!("MongoDB list collections: {}", e))?;
            coll_names.sort();
            Ok(coll_names.into_iter().map(|name| DbTableInfo { name, schema: None }).collect())
        }
        DbPool::None => Err("Not connected".to_string()),
    }
}

#[tauri::command]
pub async fn db_get_columns(
    state: tauri::State<'_, DbManager>,
    connection_id: String,
    table: String,
) -> Result<Vec<ColumnInfo>, String> {
    let pool_clone = {
        let connections = state.connections.lock().unwrap();
        let entry = connections.get(&connection_id).ok_or("Connection not found")?;
        match &entry.pool {
            DbPool::Pg(p) => DbPool::Pg(p.clone()),
            DbPool::My(p) => DbPool::My(p.clone()),
            DbPool::Sq(p) => DbPool::Sq(p.clone()),
            DbPool::Mongo(c) => DbPool::Mongo(c.clone()),
            DbPool::None => return Err("Not connected".to_string()),
        }
    };

    match pool_clone {
        DbPool::Pg(p) => {
            let rows = sqlx::query(
                "SELECT column_name, data_type, is_nullable, COALESCE((SELECT true FROM information_schema.table_constraints tc JOIN information_schema.key_column_usage kcu ON tc.constraint_name = kcu.constraint_name WHERE tc.table_name = $1 AND kcu.column_name = c.column_name AND tc.constraint_type = 'PRIMARY KEY'), false) as is_pk FROM information_schema.columns c WHERE table_name = $1 ORDER BY ordinal_position"
            )
                .bind(&table)
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            Ok(rows.iter().map(|r| {
                let name: String = r.get::<String, _>(0);
                let data_type: String = r.get::<String, _>(1);
                let nullable_str: String = r.get::<String, _>(2);
                let is_pk: bool = r.get::<bool, _>(3);
                ColumnInfo { name, data_type, nullable: nullable_str == "YES", is_pk }
            }).collect())
        }
        DbPool::My(p) => {
            let rows = sqlx::query(
                "SELECT COLUMN_NAME, DATA_TYPE, IS_NULLABLE, COLUMN_KEY = 'PRI' as is_pk FROM information_schema.columns WHERE table_name = ? AND table_schema = DATABASE() ORDER BY ordinal_position"
            )
                .bind(&table)
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            Ok(rows.iter().map(|r| {
                let name: String = r.get::<String, _>(0);
                let data_type: String = r.get::<String, _>(1);
                let nullable_str: String = r.get::<String, _>(2);
                let is_pk: bool = r.get::<bool, _>(3);
                ColumnInfo { name, data_type, nullable: nullable_str == "YES", is_pk }
            }).collect())
        }
        DbPool::Sq(p) => {
            let query_str = format!("PRAGMA table_info({})", table);
            let rows = sqlx::query(&query_str)
                .fetch_all(&p)
                .await
                .map_err(|e| format!("Query failed: {}", e))?;
            Ok(rows.iter().map(|r| {
                let name: String = r.get::<String, _>(1);
                let data_type: String = r.get::<String, _>(2);
                let not_null: i32 = r.get::<i32, _>(3);
                let nullable = not_null == 0;
                let is_pk_val: i32 = r.get::<i32, _>(5);
                let is_pk = is_pk_val == 1;
                ColumnInfo { name, data_type, nullable, is_pk }
            }).collect())
        }
        DbPool::Mongo(_) => {
            // MongoDB collections are schemaless; return empty
            Ok(vec![])
        }
        DbPool::None => Err("Not connected".to_string()),
    }
}
