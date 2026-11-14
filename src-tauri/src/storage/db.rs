use rusqlite::{params, Connection, Result};
use std::path::PathBuf;
use std::sync::Mutex;
use bandwidthx_common::{NetworkRule, RuleType, Direction};

pub struct Database {
    conn: Mutex<Connection>,
}

impl Database {
    pub fn new(db_path: PathBuf) -> Result<Self> {
        let conn = Connection::open(db_path)?;
        let db = Database { conn: Mutex::new(conn) };
        db.init_schema()?;
        Ok(db)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS rules (
                id TEXT PRIMARY KEY,
                app_name TEXT NOT NULL,
                executable_path TEXT NOT NULL,
                rule_type TEXT NOT NULL,
                direction TEXT NOT NULL,
                limit_bytes_per_sec INTEGER,
                enabled INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS analytics (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                app_name TEXT NOT NULL,
                executable_path TEXT NOT NULL,
                bytes_downloaded INTEGER NOT NULL,
                bytes_uploaded INTEGER NOT NULL,
                timestamp TEXT NOT NULL
            );

            CREATE TABLE IF NOT EXISTS settings (
                key TEXT PRIMARY KEY,
                value TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_analytics_time ON analytics(timestamp);
            CREATE INDEX IF NOT EXISTS idx_rules_exe ON rules(executable_path);
            "#
        )?;
        
        Ok(())
    }

    pub fn get_rules(&self) -> Result<Vec<NetworkRule>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, app_name, executable_path, rule_type, direction, limit_bytes_per_sec, enabled, created_at, updated_at FROM rules")?;
        
        let rule_iter = stmt.query_map([], |row| {
            let rule_type_str: String = row.get(3)?;
            let direction_str: String = row.get(4)?;
            let limit: Option<i64> = row.get(5)?;
            let enabled_int: i32 = row.get(6)?;

            let rule_type = match rule_type_str.as_str() {
                "allow" => RuleType::Allow,
                "block" => RuleType::Block,
                "download_limit" => RuleType::DownloadLimit,
                "upload_limit" => RuleType::UploadLimit,
                "bidirectional_limit" => RuleType::BidirectionalLimit,
                "priority" => RuleType::Priority,
                _ => RuleType::ExclusiveExemption,
            };

            let direction = match direction_str.as_str() {
                "inbound" => Direction::Inbound,
                "outbound" => Direction::Outbound,
                _ => Direction::Both,
            };

            Ok(NetworkRule {
                id: row.get(0)?,
                app_name: row.get(1)?,
                executable_path: row.get(2)?,
                rule_type,
                direction,
                limit_bytes_per_sec: limit.map(|l| l as u64),
                enabled: enabled_int == 1,
                created_at: row.get(7)?,
                updated_at: row.get(8)?,
            })
        })?;

        let mut rules = Vec::new();
        for r in rule_iter {
            if let Ok(rule) = r {
                rules.push(rule);
            }
        }
        Ok(rules)
    }

    pub fn save_rule(&self, rule: &NetworkRule) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let rule_type_str = match rule.rule_type {
            RuleType::Allow => "allow",
            RuleType::Block => "block",
            RuleType::DownloadLimit => "download_limit",
            RuleType::UploadLimit => "upload_limit",
            RuleType::BidirectionalLimit => "bidirectional_limit",
            RuleType::Priority => "priority",
            RuleType::ExclusiveExemption => "exclusive_exemption",
        };

        let direction_str = match rule.direction {
            Direction::Inbound => "inbound",
            Direction::Outbound => "outbound",
            Direction::Both => "both",
        };

        conn.execute(
            r#"
            INSERT INTO rules (id, app_name, executable_path, rule_type, direction, limit_bytes_per_sec, enabled, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            ON CONFLICT(id) DO UPDATE SET
                app_name=excluded.app_name,
                executable_path=excluded.executable_path,
                rule_type=excluded.rule_type,
                direction=excluded.direction,
                limit_bytes_per_sec=excluded.limit_bytes_per_sec,
                enabled=excluded.enabled,
                updated_at=excluded.updated_at
            "#,
            params![
                rule.id,
                rule.app_name,
                rule.executable_path,
                rule_type_str,
                direction_str,
                rule.limit_bytes_per_sec.map(|l| l as i64),
                if rule.enabled { 1 } else { 0 },
                rule.created_at,
                rule.updated_at
            ]
        )?;

        Ok(())
    }

    pub fn delete_rule(&self, rule_id: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let rows = conn.execute("DELETE FROM rules WHERE id = ?1", params![rule_id])?;
        Ok(rows > 0)
    }

    pub fn log_analytics(&self, app_name: &str, exe_path: &str, down_bytes: u64, up_bytes: u64) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        conn.execute(
            "INSERT INTO analytics (app_name, executable_path, bytes_downloaded, bytes_uploaded, timestamp) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![app_name, exe_path, down_bytes as i64, up_bytes as i64, now]
        )?;
        Ok(())
    }

    pub fn get_analytics(&self, _range: &str) -> Result<Vec<serde_json::Value>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            r#"
            SELECT app_name, executable_path, SUM(bytes_downloaded), SUM(bytes_uploaded), MAX(timestamp)
            FROM analytics
            GROUP BY app_name, executable_path
            ORDER BY (SUM(bytes_downloaded) + SUM(bytes_uploaded)) DESC
            LIMIT 50
            "#
        )?;

        let iter = stmt.query_map([], |row| {
            let down: i64 = row.get(2).unwrap_or(0);
            let up: i64 = row.get(3).unwrap_or(0);
            Ok(serde_json::json!({
                "app_name": row.get::<_, String>(0)?,
                "executable_path": row.get::<_, String>(1)?,
                "bytes_downloaded": down as u64,
                "bytes_uploaded": up as u64,
                "timestamp": row.get::<_, String>(4).unwrap_or_default(),
            }))
        })?;

        let mut res = Vec::new();
        for item in iter {
            if let Ok(v) = item {
                res.push(v);
            }
        }
        Ok(res)
    }

    pub fn export_csv(&self) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT app_name, executable_path, bytes_downloaded, bytes_uploaded, timestamp FROM analytics ORDER BY id DESC LIMIT 5000")?;
        let mut csv = String::from("Application,ExecutablePath,BytesDownloaded,BytesUploaded,Timestamp\n");

        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            let app: String = row.get(0)?;
            let path: String = row.get(1)?;
            let down: i64 = row.get(2)?;
            let up: i64 = row.get(3)?;
            let time: String = row.get(4)?;
            csv.push_str(&format!("\"{}\",\"{}\",{},{},\"{}\"\n", app.replace('"', "\"\""), path.replace('"', "\"\""), down, up, time));
        }

        Ok(csv)
    }
}
