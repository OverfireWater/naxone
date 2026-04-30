use std::path::Path;
use std::sync::Arc;

use crate::error::Result;
use crate::ports::config_io::ConfigIO;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct ConfigEditor {
    config_io: Arc<dyn ConfigIO>,
}

// ======================== Nginx ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NginxConfig {
    pub worker_processes: String,
    pub worker_connections: u32,
    pub worker_rlimit_nofile: u32,
    pub keepalive_timeout: u32,
    pub keepalive_requests: u32,
    pub sendfile: bool,
    pub sendfile_max_chunk: String,
    pub client_max_body_size: String,
    pub client_body_buffer_size: String,
    pub client_body_timeout: u32,
    pub client_header_buffer_size: String,
    pub client_header_timeout: u32,
    pub large_client_header_buffers: String,
    pub send_timeout: u32,
    pub reset_timedout_connection: bool,
    pub server_names_hash_bucket_size: u32,
    pub gzip: bool,
    pub gzip_level: u8,
    pub gzip_min_length: u32,
}

impl Default for NginxConfig {
    fn default() -> Self {
        Self {
            worker_processes: "4".into(),
            worker_connections: 40960,
            worker_rlimit_nofile: 100000,
            keepalive_timeout: 3600,
            keepalive_requests: 100,
            sendfile: true,
            sendfile_max_chunk: "512k".into(),
            client_max_body_size: "50m".into(),
            client_body_buffer_size: "60k".into(),
            client_body_timeout: 60,
            client_header_buffer_size: "64k".into(),
            client_header_timeout: 3600,
            large_client_header_buffers: "4 64k".into(),
            send_timeout: 3600,
            reset_timedout_connection: true,
            server_names_hash_bucket_size: 256,
            gzip: false,
            gzip_level: 6,
            gzip_min_length: 1024,
        }
    }
}

// ======================== MySQL ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MysqlConfig {
    pub port: u16,
    pub max_connections: u32,
    pub max_allowed_packet: String,
    pub innodb_buffer_pool_size: String,
    pub innodb_log_file_size: String,
    pub innodb_log_buffer_size: String,
    pub innodb_flush_log_at_trx_commit: u8,
    pub innodb_lock_wait_timeout: u32,
    pub character_set_server: String,
    pub collation_server: String,
    pub init_connect: String,
    pub key_buffer_size: String,
    pub table_open_cache: u32,
    pub tmp_table_size: String,
    pub max_heap_table_size: String,
    pub interactive_timeout: u32,
    pub wait_timeout: u32,
    pub sort_buffer_size: String,
    pub read_buffer_size: String,
    pub read_rnd_buffer_size: String,
    pub join_buffer_size: String,
    pub myisam_sort_buffer_size: String,
    pub thread_cache_size: u32,
    pub log_error_verbosity: u8,
    pub default_storage_engine: String,
}

impl Default for MysqlConfig {
    fn default() -> Self {
        Self {
            port: 3306,
            max_connections: 100,
            max_allowed_packet: "16M".into(),
            innodb_buffer_pool_size: "64M".into(),
            innodb_log_file_size: "256M".into(),
            innodb_log_buffer_size: "4M".into(),
            innodb_flush_log_at_trx_commit: 1,
            innodb_lock_wait_timeout: 120,
            character_set_server: "utf8".into(),
            collation_server: "utf8_unicode_ci".into(),
            init_connect: "SET NAMES utf8".into(),
            key_buffer_size: "32M".into(),
            table_open_cache: 256,
            tmp_table_size: "64M".into(),
            max_heap_table_size: "64M".into(),
            interactive_timeout: 120,
            wait_timeout: 120,
            sort_buffer_size: "256kb".into(),
            read_buffer_size: "512kb".into(),
            read_rnd_buffer_size: "4M".into(),
            join_buffer_size: "2M".into(),
            myisam_sort_buffer_size: "32M".into(),
            thread_cache_size: 16,
            log_error_verbosity: 1,
            default_storage_engine: "MyISAM".into(),
        }
    }
}

// ======================== Redis ========================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedisConfig {
    pub port: u16,
    pub bind: String,
    pub timeout: u32,
    pub databases: u16,
    pub maxmemory: String,
    pub maxclients: u32,
    pub maxmemory_policy: String,
    pub tcp_backlog: u32,
    pub tcp_keepalive: u32,
    pub loglevel: String,
    pub save_rules: String,
    pub rdbcompression: bool,
    pub appendonly: bool,
    pub appendfsync: String,
    pub requirepass: String,
    pub logfile: String,
    pub rdbchecksum: bool,
    pub dbfilename: String,
    pub dir: String,
    pub stop_writes_on_bgsave_error: bool,
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            port: 6379,
            bind: "127.0.0.1".into(),
            timeout: 0,
            databases: 16,
            maxmemory: "1048576000".into(),
            maxclients: 10000,
            maxmemory_policy: "noeviction".into(),
            tcp_backlog: 511,
            tcp_keepalive: 0,
            loglevel: "notice".into(),
            save_rules: "900 1 300 10 60 10000".into(),
            rdbcompression: true,
            appendonly: false,
            appendfsync: "everysec".into(),
            requirepass: String::new(),
            logfile: String::new(),
            rdbchecksum: true,
            dbfilename: "dump.rdb".into(),
            dir: "./".into(),
            stop_writes_on_bgsave_error: true,
        }
    }
}

impl ConfigEditor {
    pub fn new(config_io: Arc<dyn ConfigIO>) -> Self {
        Self { config_io }
    }

    // ======================== Nginx ========================

    pub fn read_nginx(&self, conf_path: &Path) -> Result<NginxConfig> {
        let content = self.config_io.read_text(conf_path)?;
        let mut cfg = NginxConfig::default();

        let ng = |k: &str| Self::nginx_get(&content, k);
        let ng_u32 = |k: &str, d: u32| -> u32 { ng(k).and_then(|v| v.parse().ok()).unwrap_or(d) };
        let ng_bool = |k: &str, d: bool| -> bool { ng(k).map(|v| v == "on").unwrap_or(d) };

        cfg.worker_processes = ng("worker_processes").unwrap_or(cfg.worker_processes);
        cfg.worker_connections = ng_u32("worker_connections", cfg.worker_connections);
        cfg.worker_rlimit_nofile = ng_u32("worker_rlimit_nofile", cfg.worker_rlimit_nofile);
        cfg.keepalive_timeout = ng_u32("keepalive_timeout", cfg.keepalive_timeout);
        cfg.keepalive_requests = ng_u32("keepalive_requests", cfg.keepalive_requests);
        cfg.sendfile = ng_bool("sendfile", cfg.sendfile);
        cfg.sendfile_max_chunk = ng("sendfile_max_chunk").unwrap_or(cfg.sendfile_max_chunk);
        cfg.client_max_body_size = ng("client_max_body_size").unwrap_or(cfg.client_max_body_size);
        cfg.client_body_buffer_size = ng("client_body_buffer_size").unwrap_or(cfg.client_body_buffer_size);
        cfg.client_body_timeout = ng_u32("client_body_timeout", cfg.client_body_timeout);
        cfg.client_header_buffer_size = ng("client_header_buffer_size").unwrap_or(cfg.client_header_buffer_size);
        cfg.client_header_timeout = ng_u32("client_header_timeout", cfg.client_header_timeout);
        cfg.large_client_header_buffers = ng("large_client_header_buffers").unwrap_or(cfg.large_client_header_buffers);
        cfg.send_timeout = ng_u32("send_timeout", cfg.send_timeout);
        cfg.reset_timedout_connection = ng_bool("reset_timedout_connection", cfg.reset_timedout_connection);
        cfg.server_names_hash_bucket_size = ng_u32("server_names_hash_bucket_size", cfg.server_names_hash_bucket_size);
        cfg.gzip = ng_bool("gzip", cfg.gzip);
        cfg.gzip_level = ng("gzip_level").and_then(|v| v.parse().ok()).unwrap_or(cfg.gzip_level);
        cfg.gzip_min_length = ng_u32("gzip_min_length", cfg.gzip_min_length);

        Ok(cfg)
    }

    pub fn save_nginx(&self, conf_path: &Path, cfg: &NginxConfig) -> Result<()> {
        self.config_io.backup(conf_path)?;
        let mut content = self.config_io.read_text(conf_path)?;

        let s = |c: String, k: &str, v: &str| -> String { Self::nginx_set(&c, k, v) };
        let sb = |c: String, k: &str, v: bool| -> String { Self::nginx_set(&c, k, if v { "on" } else { "off" }) };
        let si = |c: String, k: &str, v: &str, a: &str| -> String { Self::nginx_set_or_insert(&c, k, v, a) };

        content = s(content, "worker_processes", &cfg.worker_processes);
        content = s(content, "worker_connections", &cfg.worker_connections.to_string());
        content = s(content, "worker_rlimit_nofile", &cfg.worker_rlimit_nofile.to_string());
        content = s(content, "keepalive_timeout", &cfg.keepalive_timeout.to_string());
        content = s(content, "keepalive_requests", &cfg.keepalive_requests.to_string());
        content = sb(content, "sendfile", cfg.sendfile);
        content = s(content, "sendfile_max_chunk", &cfg.sendfile_max_chunk);
        content = s(content, "client_max_body_size", &cfg.client_max_body_size);
        content = s(content, "client_body_buffer_size", &cfg.client_body_buffer_size);
        content = s(content, "client_body_timeout", &cfg.client_body_timeout.to_string());
        content = s(content, "client_header_buffer_size", &cfg.client_header_buffer_size);
        content = s(content, "client_header_timeout", &cfg.client_header_timeout.to_string());
        content = s(content, "large_client_header_buffers", &cfg.large_client_header_buffers);
        content = s(content, "send_timeout", &cfg.send_timeout.to_string());
        content = sb(content, "reset_timedout_connection", cfg.reset_timedout_connection);
        content = s(content, "server_names_hash_bucket_size", &cfg.server_names_hash_bucket_size.to_string());
        content = sb(content, "gzip", cfg.gzip);
        content = si(content, "gzip_level", &cfg.gzip_level.to_string(), "gzip");
        content = si(content, "gzip_min_length", &cfg.gzip_min_length.to_string(), "gzip");

        self.config_io.write_text(conf_path, &content)
    }

    // ======================== MySQL ========================

    pub fn read_mysql(&self, conf_path: &Path) -> Result<MysqlConfig> {
        let content = self.config_io.read_text(conf_path)?;
        let mut cfg = MysqlConfig::default();

        let ig = |k: &str| Self::ini_get(&content, k);
        let ig_u32 = |k: &str, d: u32| -> u32 { ig(k).and_then(|v| v.parse().ok()).unwrap_or(d) };

        cfg.port = ig("port").and_then(|v| v.parse().ok()).unwrap_or(cfg.port);
        cfg.max_connections = ig_u32("max_connections", cfg.max_connections);
        cfg.max_allowed_packet = ig("max_allowed_packet").unwrap_or(cfg.max_allowed_packet);
        cfg.innodb_buffer_pool_size = ig("innodb_buffer_pool_size").unwrap_or(cfg.innodb_buffer_pool_size);
        cfg.innodb_log_file_size = ig("innodb_log_file_size").unwrap_or(cfg.innodb_log_file_size);
        cfg.innodb_log_buffer_size = ig("innodb_log_buffer_size").unwrap_or(cfg.innodb_log_buffer_size);
        cfg.innodb_flush_log_at_trx_commit = ig("innodb_flush_log_at_trx_commit").and_then(|v| v.parse().ok()).unwrap_or(cfg.innodb_flush_log_at_trx_commit);
        cfg.innodb_lock_wait_timeout = ig_u32("innodb_lock_wait_timeout", cfg.innodb_lock_wait_timeout);
        cfg.character_set_server = ig("character-set-server").unwrap_or(cfg.character_set_server);
        cfg.collation_server = ig("collation-server").unwrap_or(cfg.collation_server);
        cfg.init_connect = ig("init_connect").unwrap_or(cfg.init_connect);
        cfg.key_buffer_size = ig("key_buffer_size").unwrap_or(cfg.key_buffer_size);
        cfg.table_open_cache = ig_u32("table_open_cache", cfg.table_open_cache);
        cfg.tmp_table_size = ig("tmp_table_size").unwrap_or(cfg.tmp_table_size);
        cfg.max_heap_table_size = ig("max_heap_table_size").unwrap_or(cfg.max_heap_table_size);
        cfg.interactive_timeout = ig_u32("interactive_timeout", cfg.interactive_timeout);
        cfg.wait_timeout = ig_u32("wait_timeout", cfg.wait_timeout);
        cfg.sort_buffer_size = ig("sort_buffer_size").unwrap_or(cfg.sort_buffer_size);
        cfg.read_buffer_size = ig("read_buffer_size").unwrap_or(cfg.read_buffer_size);
        cfg.read_rnd_buffer_size = ig("read_rnd_buffer_size").unwrap_or(cfg.read_rnd_buffer_size);
        cfg.join_buffer_size = ig("join_buffer_size").unwrap_or(cfg.join_buffer_size);
        cfg.myisam_sort_buffer_size = ig("myisam_sort_buffer_size").unwrap_or(cfg.myisam_sort_buffer_size);
        cfg.thread_cache_size = ig_u32("thread_cache_size", cfg.thread_cache_size);
        cfg.log_error_verbosity = ig("log_error_verbosity").and_then(|v| v.parse().ok()).unwrap_or(cfg.log_error_verbosity);
        cfg.default_storage_engine = ig("default-storage-engine").unwrap_or(cfg.default_storage_engine);

        Ok(cfg)
    }

    pub fn save_mysql(&self, conf_path: &Path, cfg: &MysqlConfig) -> Result<()> {
        self.config_io.backup(conf_path)?;
        let mut content = self.config_io.read_text(conf_path)?;

        let is = |c: String, k: &str, v: &str| -> String { Self::ini_set(&c, k, v) };

        content = is(content, "port", &cfg.port.to_string());
        content = is(content, "max_connections", &cfg.max_connections.to_string());
        content = is(content, "max_allowed_packet", &cfg.max_allowed_packet);
        content = is(content, "innodb_buffer_pool_size", &cfg.innodb_buffer_pool_size);
        content = is(content, "innodb_log_file_size", &cfg.innodb_log_file_size);
        content = is(content, "innodb_log_buffer_size", &cfg.innodb_log_buffer_size);
        content = is(content, "innodb_flush_log_at_trx_commit", &cfg.innodb_flush_log_at_trx_commit.to_string());
        content = is(content, "innodb_lock_wait_timeout", &cfg.innodb_lock_wait_timeout.to_string());
        content = is(content, "character-set-server", &cfg.character_set_server);
        content = is(content, "collation-server", &cfg.collation_server);
        content = is(content, "init_connect", &cfg.init_connect);
        content = is(content, "key_buffer_size", &cfg.key_buffer_size);
        content = is(content, "table_open_cache", &cfg.table_open_cache.to_string());
        content = is(content, "tmp_table_size", &cfg.tmp_table_size);
        content = is(content, "max_heap_table_size", &cfg.max_heap_table_size);
        content = is(content, "interactive_timeout", &cfg.interactive_timeout.to_string());
        content = is(content, "wait_timeout", &cfg.wait_timeout.to_string());
        content = is(content, "sort_buffer_size", &cfg.sort_buffer_size);
        content = is(content, "read_buffer_size", &cfg.read_buffer_size);
        content = is(content, "read_rnd_buffer_size", &cfg.read_rnd_buffer_size);
        content = is(content, "join_buffer_size", &cfg.join_buffer_size);
        content = is(content, "myisam_sort_buffer_size", &cfg.myisam_sort_buffer_size);
        content = is(content, "thread_cache_size", &cfg.thread_cache_size.to_string());
        content = is(content, "log_error_verbosity", &cfg.log_error_verbosity.to_string());
        content = is(content, "default-storage-engine", &cfg.default_storage_engine);

        self.config_io.write_text(conf_path, &content)
    }

    // ======================== Redis ========================

    pub fn read_redis(&self, conf_path: &Path) -> Result<RedisConfig> {
        let content = self.config_io.read_text(conf_path)?;
        let mut cfg = RedisConfig::default();

        cfg.port = Self::redis_get(&content, "port").and_then(|v| v.parse().ok()).unwrap_or(cfg.port);
        cfg.bind = Self::redis_get(&content, "bind").unwrap_or(cfg.bind);
        cfg.timeout = Self::redis_get(&content, "timeout").and_then(|v| v.parse().ok()).unwrap_or(cfg.timeout);
        cfg.databases = Self::redis_get(&content, "databases").and_then(|v| v.parse().ok()).unwrap_or(cfg.databases);
        cfg.maxmemory = Self::redis_get(&content, "maxmemory").unwrap_or(cfg.maxmemory);
        cfg.maxclients = Self::redis_get(&content, "maxclients").and_then(|v| v.parse().ok()).unwrap_or(cfg.maxclients);
        cfg.maxmemory_policy = Self::redis_get(&content, "maxmemory-policy").unwrap_or(cfg.maxmemory_policy);
        cfg.tcp_backlog = Self::redis_get(&content, "tcp-backlog").and_then(|v| v.parse().ok()).unwrap_or(cfg.tcp_backlog);
        cfg.tcp_keepalive = Self::redis_get(&content, "tcp-keepalive").and_then(|v| v.parse().ok()).unwrap_or(cfg.tcp_keepalive);
        cfg.loglevel = Self::redis_get(&content, "loglevel").unwrap_or(cfg.loglevel);
        cfg.rdbcompression = Self::redis_get(&content, "rdbcompression").map(|v| v == "yes").unwrap_or(cfg.rdbcompression);
        cfg.appendonly = Self::redis_get(&content, "appendonly").map(|v| v == "yes").unwrap_or(cfg.appendonly);
        cfg.appendfsync = Self::redis_get(&content, "appendfsync").unwrap_or(cfg.appendfsync);
        cfg.requirepass = Self::redis_get(&content, "requirepass").unwrap_or(cfg.requirepass);
        cfg.logfile = Self::redis_get(&content, "logfile").unwrap_or(cfg.logfile);
        cfg.rdbchecksum = Self::redis_get(&content, "rdbchecksum").map(|v| v == "yes").unwrap_or(cfg.rdbchecksum);
        cfg.dbfilename = Self::redis_get(&content, "dbfilename").unwrap_or(cfg.dbfilename);
        cfg.dir = Self::redis_get(&content, "dir").unwrap_or(cfg.dir);
        cfg.stop_writes_on_bgsave_error = Self::redis_get(&content, "stop-writes-on-bgsave-error").map(|v| v == "yes").unwrap_or(cfg.stop_writes_on_bgsave_error);

        // Collect save rules
        let saves: Vec<String> = content
            .lines()
            .filter(|l| {
                let t = l.trim();
                t.starts_with("save ") && !t.starts_with('#')
            })
            .map(|l| l.trim().strip_prefix("save ").unwrap_or("").to_string())
            .collect();
        if !saves.is_empty() {
            cfg.save_rules = saves.join(" / ");
        }

        Ok(cfg)
    }

    pub fn save_redis(&self, conf_path: &Path, cfg: &RedisConfig) -> Result<()> {
        self.config_io.backup(conf_path)?;
        let mut content = self.config_io.read_text(conf_path)?;

        content = Self::redis_set(&content, "port", &cfg.port.to_string());
        content = Self::redis_set(&content, "bind", &cfg.bind);
        content = Self::redis_set(&content, "timeout", &cfg.timeout.to_string());
        content = Self::redis_set(&content, "databases", &cfg.databases.to_string());
        content = Self::redis_set(&content, "maxmemory", &cfg.maxmemory);
        content = Self::redis_set(&content, "maxclients", &cfg.maxclients.to_string());
        content = Self::redis_set(&content, "maxmemory-policy", &cfg.maxmemory_policy);
        content = Self::redis_set(&content, "tcp-backlog", &cfg.tcp_backlog.to_string());
        content = Self::redis_set(&content, "tcp-keepalive", &cfg.tcp_keepalive.to_string());
        content = Self::redis_set(&content, "loglevel", &cfg.loglevel);
        content = Self::redis_set(&content, "rdbcompression", if cfg.rdbcompression { "yes" } else { "no" });
        content = Self::redis_set(&content, "appendonly", if cfg.appendonly { "yes" } else { "no" });
        content = Self::redis_set(&content, "appendfsync", &cfg.appendfsync);

        // Handle requirepass specially (comment out if empty)
        if cfg.requirepass.is_empty() {
            content = Self::redis_comment(&content, "requirepass");
        } else {
            content = Self::redis_set(&content, "requirepass", &cfg.requirepass);
        }

        content = Self::redis_set(&content, "rdbchecksum", if cfg.rdbchecksum { "yes" } else { "no" });
        content = Self::redis_set(&content, "dbfilename", &cfg.dbfilename);
        content = Self::redis_set(&content, "dir", &cfg.dir);
        content = Self::redis_set(&content, "stop-writes-on-bgsave-error", if cfg.stop_writes_on_bgsave_error { "yes" } else { "no" });

        self.config_io.write_text(conf_path, &content)
    }

    // ======================== Parsers ========================

    // --- Nginx: `directive value;` ---
    fn nginx_get(content: &str, key: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { continue; }
            if let Some(rest) = trimmed.strip_prefix(key) {
                let rest = rest.trim();
                if rest.is_empty() { continue; }
                // Must be followed by whitespace (not part of a longer directive name)
                if rest.starts_with(|c: char| c.is_whitespace()) || rest.starts_with(';') {
                    return Some(rest.trim().trim_end_matches(';').trim().to_string());
                }
            }
        }
        None
    }

    fn nginx_set(content: &str, key: &str, value: &str) -> String {
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { continue; }
            if let Some(rest) = trimmed.strip_prefix(key) {
                if rest.trim().starts_with(|c: char| c.is_whitespace()) || rest.trim().starts_with(';') {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    *line = format!("{}{} {};", indent, key, value);
                    return lines.join("\n");
                }
            }
        }
        lines.join("\n")
    }

    fn nginx_set_or_insert(content: &str, key: &str, value: &str, after_key: &str) -> String {
        // Try to set existing
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { continue; }
            if let Some(rest) = trimmed.strip_prefix(key) {
                if rest.trim().starts_with(|c: char| c.is_whitespace()) || rest.trim().starts_with(';') {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    *line = format!("{}{} {};", indent, key, value);
                    return lines.join("\n");
                }
            }
        }
        // Not found: also check commented lines
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if let Some(uncommented) = trimmed.strip_prefix('#') {
                let uncommented = uncommented.trim();
                if uncommented.starts_with(key) {
                    let indent = &line[..line.len() - line.trim_start().len()];
                    *line = format!("{}{} {};", indent, key, value);
                    return lines.join("\n");
                }
            }
        }
        // Insert after after_key
        let insert_pos = lines.iter().rposition(|l| {
            let t = l.trim();
            !t.starts_with('#') && t.contains(after_key)
        }).map(|p| p + 1);
        if let Some(pos) = insert_pos {
            lines.insert(pos, format!("    {} {};", key, value));
        }
        lines.join("\n")
    }

    // --- INI format: `key=value` or `key = value` ---
    fn ini_get(content: &str, key: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') || trimmed.is_empty() {
                continue;
            }
            if let Some((k, v)) = trimmed.split_once('=') {
                if k.trim() == key {
                    return Some(v.trim().to_string());
                }
            }
        }
        None
    }

    fn ini_set(content: &str, key: &str, value: &str) -> String {
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.starts_with(';') { continue; }
            if let Some((k, _)) = trimmed.split_once('=') {
                if k.trim() == key {
                    *line = format!("{}={}", key, value);
                    return lines.join("\n");
                }
            }
        }
        // Not found, try to uncomment
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if let Some(uncommented) = trimmed.strip_prefix('#').or_else(|| trimmed.strip_prefix(';')) {
                if let Some((k, _)) = uncommented.trim().split_once('=') {
                    if k.trim() == key {
                        *line = format!("{}={}", key, value);
                        return lines.join("\n");
                    }
                }
            }
        }
        lines.join("\n")
    }

    // --- Redis format: `key value` ---
    fn redis_get(content: &str, key: &str) -> Option<String> {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') || trimmed.is_empty() { continue; }
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            if let (Some(k), Some(v)) = (parts.next(), parts.next()) {
                if k == key {
                    let v = v.trim().trim_matches('"');
                    return Some(v.to_string());
                }
            }
        }
        None
    }

    fn redis_set(content: &str, key: &str, value: &str) -> String {
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { continue; }
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            if let Some(k) = parts.next() {
                if k == key {
                    *line = format!("{} {}", key, value);
                    return lines.join("\n");
                }
            }
        }
        // Try uncommenting
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if let Some(uncommented) = trimmed.strip_prefix('#') {
                let uncommented = uncommented.trim();
                let mut parts = uncommented.splitn(2, char::is_whitespace);
                if let Some(k) = parts.next() {
                    if k == key {
                        *line = format!("{} {}", key, value);
                        return lines.join("\n");
                    }
                }
            }
        }
        // Append
        lines.push(format!("{} {}", key, value));
        lines.join("\n")
    }

    fn redis_comment(content: &str, key: &str) -> String {
        let mut lines: Vec<String> = content.lines().map(|l| l.to_string()).collect();
        for line in lines.iter_mut() {
            let trimmed = line.trim();
            if trimmed.starts_with('#') { continue; }
            let mut parts = trimmed.splitn(2, char::is_whitespace);
            if let Some(k) = parts.next() {
                if k == key {
                    *line = format!("# {}", trimmed);
                    return lines.join("\n");
                }
            }
        }
        lines.join("\n")
    }
}
