<script setup lang="ts">
import { ref, watch, onMounted, computed } from "vue";
import { useRoute, useRouter } from "vue-router";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "../composables/useToast";
import SelectMenu from "../components/SelectMenu.vue";
import GlobalEnv from "./GlobalEnv.vue";

type Tab = "nginx" | "mysql" | "redis" | "php" | "hosts" | "env";
const route = useRoute();
const router = useRouter();
const activeTab = ref<Tab>("env");

function setTab(t: Tab) {
  activeTab.value = t;
  // 同步到 URL；env 是默认 tab，省略 query 让 URL 更干净
  router.replace({ path: "/config", query: t === "env" ? {} : { tab: t } });
}
const busy = ref(false);
const saved = ref(false);
const boolOptions = [
  { label: "开启", value: true },
  { label: "关闭", value: false },
];
const mysqlFlushOptions = [
  { label: "0 - 每秒刷新", value: 0 },
  { label: "1 - 每次提交(最安全)", value: 1 },
  { label: "2 - 写日志每秒刷盘", value: 2 },
];
const mysqlCharsetOptions = [
  { label: "utf8", value: "utf8" },
  { label: "utf8mb4", value: "utf8mb4" },
  { label: "latin1", value: "latin1" },
  { label: "gbk", value: "gbk" },
];
const mysqlEngineOptions = [
  { label: "InnoDB", value: "InnoDB" },
  { label: "MyISAM", value: "MyISAM" },
];
const mysqlLogVerbosityOptions = [
  { label: "1 - 仅错误", value: 1 },
  { label: "2 - 错误+警告", value: 2 },
  { label: "3 - 全部", value: 3 },
];
const redisPolicyOptions = [
  { label: "noeviction", value: "noeviction" },
  { label: "allkeys-lru", value: "allkeys-lru" },
  { label: "volatile-lru", value: "volatile-lru" },
  { label: "allkeys-random", value: "allkeys-random" },
  { label: "volatile-random", value: "volatile-random" },
  { label: "volatile-ttl", value: "volatile-ttl" },
];
const redisAppendfsyncOptions = [
  { label: "everysec", value: "everysec" },
  { label: "always", value: "always" },
  { label: "no", value: "no" },
];
const redisLoglevelOptions = [
  { label: "debug", value: "debug" },
  { label: "verbose", value: "verbose" },
  { label: "notice", value: "notice" },
  { label: "warning", value: "warning" },
];
const phpSessionHandlerOptions = [
  { label: "files", value: "files" },
  { label: "redis", value: "redis" },
  { label: "memcached", value: "memcached" },
];
const phpSameSiteOptions = [
  { label: "不设置", value: "" },
  { label: "Lax", value: "Lax" },
  { label: "Strict", value: "Strict" },
  { label: "None", value: "None" },
];

function showError(msg: string) { toast.error(String(msg)); }
function showSaved() { saved.value = true; setTimeout(() => (saved.value = false), 2000); }

// ======================== Nginx ========================
interface NginxConfig {
  worker_processes: string; worker_connections: number; worker_rlimit_nofile: number;
  keepalive_timeout: number; keepalive_requests: number;
  sendfile: boolean; sendfile_max_chunk: string;
  client_max_body_size: string; client_body_buffer_size: string; client_body_timeout: number;
  client_header_buffer_size: string; client_header_timeout: number; large_client_header_buffers: string;
  send_timeout: number; reset_timedout_connection: boolean;
  server_names_hash_bucket_size: number;
  gzip: boolean; gzip_level: number; gzip_min_length: number;
}
const nginx = ref<NginxConfig | null>(null);
async function loadNginx() { try { nginx.value = await invoke("get_nginx_config"); } catch (e) { showError("加载 Nginx 配置失败: " + e); } }
async function saveNginx() { if (!nginx.value || busy.value) return; busy.value = true; try { await invoke("save_nginx_config", { cfg: nginx.value }); showSaved(); } catch (e) { showError("保存失败: " + e); } finally { busy.value = false; } }

// ======================== MySQL ========================
interface MysqlConfig {
  port: number; max_connections: number; max_allowed_packet: string;
  innodb_buffer_pool_size: string; innodb_log_file_size: string; innodb_log_buffer_size: string;
  innodb_flush_log_at_trx_commit: number; innodb_lock_wait_timeout: number;
  character_set_server: string; collation_server: string; init_connect: string;
  key_buffer_size: string; table_open_cache: number; tmp_table_size: string; max_heap_table_size: string;
  interactive_timeout: number; wait_timeout: number;
  sort_buffer_size: string; read_buffer_size: string; read_rnd_buffer_size: string;
  join_buffer_size: string; myisam_sort_buffer_size: string;
  thread_cache_size: number; log_error_verbosity: number; default_storage_engine: string;
}
const mysql = ref<MysqlConfig | null>(null);
async function loadMysql() { try { mysql.value = await invoke("get_mysql_config"); } catch (e) { showError("加载 MySQL 配置失败: " + e); } }
async function saveMysql() { if (!mysql.value || busy.value) return; busy.value = true; try { await invoke("save_mysql_config", { cfg: mysql.value }); showSaved(); } catch (e) { showError("保存失败: " + e); } finally { busy.value = false; } }

// ======================== Redis ========================
interface RedisConfig {
  port: number; bind: string; timeout: number; databases: number; maxmemory: string;
  maxclients: number; maxmemory_policy: string; tcp_backlog: number; tcp_keepalive: number;
  loglevel: string; save_rules: string; rdbcompression: boolean; appendonly: boolean;
  appendfsync: string; requirepass: string;
  logfile: string; rdbchecksum: boolean; dbfilename: string; dir: string; stop_writes_on_bgsave_error: boolean;
}
const redis = ref<RedisConfig | null>(null);
async function loadRedis() { try { redis.value = await invoke("get_redis_config"); } catch (e) { showError("加载 Redis 配置失败: " + e); } }
async function saveRedis() { if (!redis.value || busy.value) return; busy.value = true; try { await invoke("save_redis_config", { cfg: redis.value }); showSaved(); } catch (e) { showError("保存失败: " + e); } finally { busy.value = false; } }

// ======================== PHP ========================
interface PhpInstance { id: string; label: string; version: string; variant: string | null; install_path: string; }
interface PhpExtension { name: string; file_name: string; enabled: boolean; is_zend: boolean; }
interface PhpIniSettings {
  memory_limit: string; upload_max_filesize: string; post_max_size: string;
  max_execution_time: number; max_input_time: number; display_errors: boolean;
  error_reporting: string; date_timezone: string; file_uploads: boolean; short_open_tag: boolean;
  allow_url_fopen: boolean; allow_url_include: boolean; disable_functions: string;
  expose_php: boolean; open_basedir: string; opcache_enable: boolean;
  opcache_memory_consumption: number; opcache_max_accelerated_files: number;
  opcache_validate_timestamps: boolean; opcache_revalidate_freq: number;
  session_save_handler: string; session_save_path: string;
  session_gc_maxlifetime: number; session_cookie_lifetime: number;
  session_name: string; session_use_cookies: boolean; session_use_only_cookies: boolean;
  session_use_strict_mode: boolean; session_cookie_httponly: boolean; session_cookie_samesite: string;
  output_buffering: string; default_charset: string; max_file_uploads: number; default_socket_timeout: number;
}

const phpInstances = ref<PhpInstance[]>([]);
const selectedPhp = ref<PhpInstance | null>(null);
const phpExts = ref<PhpExtension[]>([]);
const phpIni = ref<PhpIniSettings | null>(null);
const phpSubTab = ref<"extensions" | "settings">("extensions");

async function loadPhpInstances() {
  try {
    phpInstances.value = await invoke("get_php_instances");
    if (phpInstances.value.length > 0 && !selectedPhp.value) selectedPhp.value = phpInstances.value[0];
  } catch (e) { showError("加载 PHP 版本失败: " + e); }
}
async function loadPhpExts() {
  if (!selectedPhp.value) return;
  try { phpExts.value = await invoke("get_php_extensions", { installPath: selectedPhp.value.install_path }); } catch (e) { showError("加载扩展失败: " + e); }
}
async function loadPhpIni() {
  if (!selectedPhp.value) return;
  try { phpIni.value = await invoke("get_php_ini_settings", { installPath: selectedPhp.value.install_path }); } catch (e) { showError("加载配置失败: " + e); }
}
async function toggleExt(ext: PhpExtension) {
  if (!selectedPhp.value || busy.value) return;
  busy.value = true;
  try { phpExts.value = await invoke("toggle_php_extension", { installPath: selectedPhp.value.install_path, extName: ext.name, enable: !ext.enabled, isZend: ext.is_zend }); } catch (e) { showError("切换失败: " + e); } finally { busy.value = false; }
}
async function savePhpIni() {
  if (!selectedPhp.value || !phpIni.value || busy.value) return;
  busy.value = true;
  try { await invoke("save_php_ini_settings", { installPath: selectedPhp.value.install_path, settings: phpIni.value }); showSaved(); } catch (e) { showError("保存失败: " + e); } finally { busy.value = false; }
}

watch(selectedPhp, () => { loadPhpExts(); loadPhpIni(); });
const phpInstanceOptions = computed(() => phpInstances.value.map((inst) => ({ label: inst.label, value: inst.id })));
function selectPhpInstance(id: string | number | boolean | null) {
  selectedPhp.value = phpInstances.value.find(i => i.id === id) || null;
}

// ======================== Hosts ========================
const hostsText = ref("");
const hostsOriginal = ref("");
const hostsPath = ref("");

function hostsDirty(): boolean {
  return hostsText.value !== hostsOriginal.value;
}

async function loadHosts() {
  try {
    hostsPath.value = await invoke<string>("get_hosts_file_path");
    const text = await invoke<string>("get_hosts_text");
    hostsText.value = text;
    hostsOriginal.value = text;
  } catch (e) {
    showError("加载 hosts 失败: " + e);
  }
}

async function saveHosts() {
  if (busy.value) return;
  busy.value = true;
  try {
    await invoke("save_hosts_text", { text: hostsText.value });
    hostsOriginal.value = hostsText.value;
    showSaved();
  } catch (e) {
    const msg = String(e ?? "");
    if (msg.startsWith("PERMISSION_DENIED:")) {
      const ok = await confirm("保存 hosts 需要管理员权限，是否继续提权保存？");
      if (!ok) return;
      await invoke("save_hosts_text_elevated", { text: hostsText.value });
      hostsOriginal.value = hostsText.value;
      showSaved();
    } else {
      showError("保存 hosts 失败: " + e);
    }
  } finally {
    busy.value = false;
  }
}

async function openHostsExternally() {
  try {
    const path = hostsPath.value || await invoke<string>("get_hosts_file_path");
    await invoke("open_file", { path });
  } catch (e) {
    showError("打开 hosts 失败: " + e);
  }
}
async function openConfigFile() {
  try {
    if (activeTab.value === "php" && selectedPhp.value) {
      await invoke("open_file", { path: selectedPhp.value.install_path + "/php.ini" });
    } else if (activeTab.value === "hosts") {
      const path = hostsPath.value || await invoke<string>("get_hosts_file_path");
      await invoke("open_file", { path });
    } else {
      const path = await invoke<string>("get_config_file_path", { service: activeTab.value });
      await invoke("open_file", { path });
    }
  } catch (e) { showError("打开失败: " + e); }
}
const logContent = ref("");
const showLog = ref(false);

async function viewLog() {
  try {
    logContent.value = await invoke("find_and_read_log", { service: activeTab.value });
    showLog.value = true;
  } catch (e) { showError("读取日志失败: " + e); }
}

function loadTab() {
  if (activeTab.value === "nginx" && !nginx.value) loadNginx();
  if (activeTab.value === "mysql" && !mysql.value) loadMysql();
  if (activeTab.value === "redis" && !redis.value) loadRedis();
  if (activeTab.value === "php" && phpInstances.value.length === 0) { loadPhpInstances(); }
  if (activeTab.value === "hosts" && !hostsPath.value) { loadHosts(); }
  // env tab 自带数据加载（GlobalEnv 组件 onMounted 拉取），这里不用做事
}
watch(activeTab, loadTab);
onMounted(() => {
  // 支持 /config?tab=xxx 直接定位
  const q = route.query.tab;
  if (typeof q === "string" && (["nginx","mysql","redis","php","hosts","env"] as const).includes(q as Tab)) {
    activeTab.value = q as Tab;
  }
  loadTab();
});
</script>

<template>
  <div class="max-w-[960px] has-save-bar">
    <div class="tabs-row">
      <button class="tab tab-env" :class="{ active: activeTab === 'env' }" @click="setTab('env')">全局环境</button>
      <button class="tab" :class="{ active: activeTab === 'nginx' }" @click="setTab('nginx')">Nginx</button>
      <button class="tab" :class="{ active: activeTab === 'mysql' }" @click="setTab('mysql')">MySQL</button>
      <button class="tab" :class="{ active: activeTab === 'redis' }" @click="setTab('redis')">Redis</button>
      <button class="tab" :class="{ active: activeTab === 'php' }" @click="setTab('php')">PHP</button>
      <button class="tab" :class="{ active: activeTab === 'hosts' }" @click="setTab('hosts')">Hosts</button>
    </div>

    <!-- ==================== Nginx ==================== -->
    <div v-if="activeTab === 'nginx' && nginx" class="tab-card p-6">
      <div class="cfg-section">进程与连接</div>
      <div class="cfg-grid">
        <div class="fg"><label>工作进程数 <span class="k">worker_processes</span></label><input class="input" v-model="nginx.worker_processes" /></div>
        <div class="fg"><label>最大连接数 <span class="k">worker_connections</span></label><input class="input" type="number" v-model.number="nginx.worker_connections" /></div>
        <div class="fg"><label>文件描述符限制 <span class="k">worker_rlimit_nofile</span></label><input class="input" type="number" v-model.number="nginx.worker_rlimit_nofile" /></div>
        <div class="fg"><label>域名哈希桶大小 <span class="k">server_names_hash_bucket_size</span></label><input class="input" type="number" v-model.number="nginx.server_names_hash_bucket_size" /></div>
      </div>
      <div class="cfg-section">超时与保活</div>
      <div class="cfg-grid">
        <div class="fg"><label>保活超时(秒) <span class="k">keepalive_timeout</span></label><input class="input" type="number" v-model.number="nginx.keepalive_timeout" /></div>
        <div class="fg"><label>单连接最大请求数 <span class="k">keepalive_requests</span></label><input class="input" type="number" v-model.number="nginx.keepalive_requests" /></div>
        <div class="fg"><label>发送超时(秒) <span class="k">send_timeout</span></label><input class="input" type="number" v-model.number="nginx.send_timeout" /></div>
        <div class="fg"><label>超时连接重置 <span class="k">reset_timedout_connection</span></label><SelectMenu v-model="nginx.reset_timedout_connection" :options="boolOptions" full-width trigger-class="input" /></div>
      </div>
      <div class="cfg-section">文件传输</div>
      <div class="cfg-grid">
        <div class="fg"><label>高效文件传输 <span class="k">sendfile</span></label><SelectMenu v-model="nginx.sendfile" :options="boolOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>单次发送最大字节 <span class="k">sendfile_max_chunk</span></label><input class="input" v-model="nginx.sendfile_max_chunk" /></div>
      </div>
      <div class="cfg-section">请求限制</div>
      <div class="cfg-grid">
        <div class="fg"><label>请求体大小限制 <span class="k">client_max_body_size</span></label><input class="input" v-model="nginx.client_max_body_size" /></div>
        <div class="fg"><label>请求体缓冲区 <span class="k">client_body_buffer_size</span></label><input class="input" v-model="nginx.client_body_buffer_size" /></div>
        <div class="fg"><label>请求体超时(秒) <span class="k">client_body_timeout</span></label><input class="input" type="number" v-model.number="nginx.client_body_timeout" /></div>
        <div class="fg"><label>请求头缓冲区 <span class="k">client_header_buffer_size</span></label><input class="input" v-model="nginx.client_header_buffer_size" /></div>
        <div class="fg"><label>请求头超时(秒) <span class="k">client_header_timeout</span></label><input class="input" type="number" v-model.number="nginx.client_header_timeout" /></div>
        <div class="fg"><label>大请求头缓冲 <span class="k">large_client_header_buffers</span></label><input class="input" v-model="nginx.large_client_header_buffers" /></div>
      </div>
      <div class="cfg-section">Gzip 压缩</div>
      <div class="cfg-grid">
        <div class="fg"><label>启用 Gzip <span class="k">gzip</span></label><SelectMenu v-model="nginx.gzip" :options="boolOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>压缩等级(1-9) <span class="k">gzip_level</span></label><input class="input" type="number" v-model.number="nginx.gzip_level" min="1" max="9" /></div>
        <div class="fg"><label>最小压缩体积(字节) <span class="k">gzip_min_length</span></label><input class="input" type="number" v-model.number="nginx.gzip_min_length" /></div>
      </div>
    </div>

    <!-- ==================== MySQL ==================== -->
    <div v-if="activeTab === 'mysql' && mysql" class="tab-card p-6">
      <div class="cfg-section">网络</div>
      <div class="cfg-grid">
        <div class="fg"><label>端口 <span class="k">port</span></label><input class="input" type="number" v-model.number="mysql.port" /></div>
        <div class="fg"><label>最大连接数 <span class="k">max_connections</span></label><input class="input" type="number" v-model.number="mysql.max_connections" /></div>
        <div class="fg"><label>交互超时(秒) <span class="k">interactive_timeout</span></label><input class="input" type="number" v-model.number="mysql.interactive_timeout" /></div>
        <div class="fg"><label>等待超时(秒) <span class="k">wait_timeout</span></label><input class="input" type="number" v-model.number="mysql.wait_timeout" /></div>
      </div>
      <div class="cfg-section">InnoDB 引擎</div>
      <div class="cfg-grid">
        <div class="fg"><label>缓冲池大小 <span class="k">innodb_buffer_pool_size</span></label><input class="input" v-model="mysql.innodb_buffer_pool_size" /></div>
        <div class="fg"><label>日志文件大小 <span class="k">innodb_log_file_size</span></label><input class="input" v-model="mysql.innodb_log_file_size" /></div>
        <div class="fg"><label>日志缓冲大小 <span class="k">innodb_log_buffer_size</span></label><input class="input" v-model="mysql.innodb_log_buffer_size" /></div>
        <div class="fg"><label>事务刷新策略 <span class="k">innodb_flush_log_at_trx_commit</span></label><SelectMenu v-model="mysql.innodb_flush_log_at_trx_commit" :options="mysqlFlushOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>锁等待超时(秒) <span class="k">innodb_lock_wait_timeout</span></label><input class="input" type="number" v-model.number="mysql.innodb_lock_wait_timeout" /></div>
      </div>
      <div class="cfg-section">缓冲与缓存</div>
      <div class="cfg-grid">
        <div class="fg"><label>最大数据包 <span class="k">max_allowed_packet</span></label><input class="input" v-model="mysql.max_allowed_packet" /></div>
        <div class="fg"><label>键缓冲区 <span class="k">key_buffer_size</span></label><input class="input" v-model="mysql.key_buffer_size" /></div>
        <div class="fg"><label>表打开缓存 <span class="k">table_open_cache</span></label><input class="input" type="number" v-model.number="mysql.table_open_cache" /></div>
        <div class="fg"><label>临时表大小 <span class="k">tmp_table_size</span></label><input class="input" v-model="mysql.tmp_table_size" /></div>
        <div class="fg"><label>内存表最大大小 <span class="k">max_heap_table_size</span></label><input class="input" v-model="mysql.max_heap_table_size" /></div>
        <div class="fg"><label>排序缓冲区 <span class="k">sort_buffer_size</span></label><input class="input" v-model="mysql.sort_buffer_size" /></div>
        <div class="fg"><label>读缓冲区 <span class="k">read_buffer_size</span></label><input class="input" v-model="mysql.read_buffer_size" /></div>
        <div class="fg"><label>随机读缓冲区 <span class="k">read_rnd_buffer_size</span></label><input class="input" v-model="mysql.read_rnd_buffer_size" /></div>
        <div class="fg"><label>JOIN 缓冲区 <span class="k">join_buffer_size</span></label><input class="input" v-model="mysql.join_buffer_size" /></div>
        <div class="fg"><label>MyISAM 排序缓冲 <span class="k">myisam_sort_buffer_size</span></label><input class="input" v-model="mysql.myisam_sort_buffer_size" /></div>
        <div class="fg"><label>线程缓存 <span class="k">thread_cache_size</span></label><input class="input" type="number" v-model.number="mysql.thread_cache_size" /></div>
      </div>
      <div class="cfg-section">字符集与存储</div>
      <div class="cfg-grid">
        <div class="fg"><label>服务端字符集 <span class="k">character-set-server</span></label><SelectMenu v-model="mysql.character_set_server" :options="mysqlCharsetOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>排序规则 <span class="k">collation-server</span></label><input class="input" v-model="mysql.collation_server" /></div>
        <div class="fg"><label>初始化连接命令 <span class="k">init_connect</span></label><input class="input" v-model="mysql.init_connect" /></div>
        <div class="fg"><label>默认存储引擎 <span class="k">default-storage-engine</span></label><SelectMenu v-model="mysql.default_storage_engine" :options="mysqlEngineOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>错误日志详细级别 <span class="k">log_error_verbosity</span></label><SelectMenu v-model="mysql.log_error_verbosity" :options="mysqlLogVerbosityOptions" full-width trigger-class="input" /></div>
      </div>
    </div>

    <!-- ==================== Redis ==================== -->
    <div v-if="activeTab === 'redis' && redis" class="tab-card p-6">
      <div class="cfg-section">网络</div>
      <div class="cfg-grid">
        <div class="fg"><label>端口 <span class="k">port</span></label><input class="input" type="number" v-model.number="redis.port" /></div>
        <div class="fg"><label>绑定地址 <span class="k">bind</span></label><input class="input" v-model="redis.bind" /></div>
        <div class="fg"><label>客户端超时(秒) <span class="k">timeout</span></label><input class="input" type="number" v-model.number="redis.timeout" /></div>
        <div class="fg"><label>最大客户端数 <span class="k">maxclients</span></label><input class="input" type="number" v-model.number="redis.maxclients" /></div>
        <div class="fg"><label>TCP Backlog <span class="k">tcp-backlog</span></label><input class="input" type="number" v-model.number="redis.tcp_backlog" /></div>
        <div class="fg"><label>TCP Keepalive <span class="k">tcp-keepalive</span></label><input class="input" type="number" v-model.number="redis.tcp_keepalive" /></div>
      </div>
      <div class="cfg-section">内存</div>
      <div class="cfg-grid">
        <div class="fg"><label>最大内存(字节) <span class="k">maxmemory</span></label><input class="input" v-model="redis.maxmemory" /></div>
        <div class="fg"><label>淘汰策略 <span class="k">maxmemory-policy</span></label><SelectMenu v-model="redis.maxmemory_policy" :options="redisPolicyOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>数据库数量 <span class="k">databases</span></label><input class="input" type="number" v-model.number="redis.databases" /></div>
      </div>
      <div class="cfg-section">持久化</div>
      <div class="cfg-grid">
        <div class="fg"><label>RDB 压缩 <span class="k">rdbcompression</span></label><SelectMenu v-model="redis.rdbcompression" :options="boolOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>RDB 校验和 <span class="k">rdbchecksum</span></label><SelectMenu v-model="redis.rdbchecksum" :options="boolOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>RDB 文件名 <span class="k">dbfilename</span></label><input class="input" v-model="redis.dbfilename" /></div>
        <div class="fg"><label>工作目录 <span class="k">dir</span></label><input class="input" v-model="redis.dir" /></div>
        <div class="fg"><label>保存失败时停止写入 <span class="k">stop-writes-on-bgsave-error</span></label><SelectMenu v-model="redis.stop_writes_on_bgsave_error" :options="boolOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>AOF 持久化 <span class="k">appendonly</span></label><SelectMenu v-model="redis.appendonly" :options="boolOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>AOF 刷盘策略 <span class="k">appendfsync</span></label><SelectMenu v-model="redis.appendfsync" :options="redisAppendfsyncOptions" full-width trigger-class="input" /></div>
      </div>
      <div class="cfg-section">安全</div>
      <div class="cfg-grid">
        <div class="fg full"><label>访问密码 <span class="k">requirepass</span> <span class="text-xs text-content-muted font-normal">留空表示无密码</span></label><input class="input" v-model="redis.requirepass" placeholder="留空则不设密码" /></div>
      </div>
      <div class="cfg-section">日志</div>
      <div class="cfg-grid">
        <div class="fg"><label>日志级别 <span class="k">loglevel</span></label><SelectMenu v-model="redis.loglevel" :options="redisLoglevelOptions" full-width trigger-class="input" /></div>
        <div class="fg"><label>日志文件 <span class="k">logfile</span></label><input class="input" v-model="redis.logfile" placeholder="留空输出到控制台" /></div>
      </div>
    </div>

    <!-- ==================== PHP ==================== -->
    <div v-if="activeTab === 'php'" class="tab-card p-6">
      <!-- PHP Version Selector -->
      <div class="flex items-center justify-between mb-5 gap-4">
        <SelectMenu v-if="phpInstances.length > 0" :model-value="selectedPhp?.id ?? null" :options="phpInstanceOptions" trigger-class="input w-[240px] shrink-0" @update:modelValue="selectPhpInstance($event)" />
        <div class="flex gap-1 shrink-0">
          <button
            v-for="st in [{k:'extensions',l:'扩展管理'},{k:'settings',l:'php.ini 配置'}]" :key="st.k"
            class="px-4 py-1.5 rounded-md text-[13px] cursor-pointer transition-all border"
            :class="phpSubTab === st.k ? 'bg-accent-blue text-white border-accent-blue' : 'bg-surface-primary text-content-muted border-border hover:text-content-secondary'"
            @click="phpSubTab = st.k as any"
          >{{ st.l }}</button>
        </div>
      </div>

      <!-- Extensions -->
      <div v-if="phpSubTab === 'extensions'">
        <div class="py-2.5 text-[13px] text-content-muted border-b border-border mb-1.5">切换开关启用/禁用扩展，修改后需重启 PHP 生效</div>
        <div v-for="ext in phpExts" :key="ext.file_name" class="flex items-center gap-3 py-2.5 border-b border-border last:border-b-0 hover:bg-surface-hover -mx-2 px-2 rounded transition-colors">
          <label class="toggle-wrap"><input type="checkbox" :checked="ext.enabled" :disabled="busy" @change="toggleExt(ext)" /><span class="toggle-slider"></span></label>
          <span class="text-sm">{{ ext.name }}</span>
          <span v-if="ext.is_zend" class="text-2xs px-2 py-0.5 rounded bg-[#3730a3] text-[#c7d2fe]">Zend</span>
        </div>
      </div>

      <!-- INI Settings -->
      <div v-if="phpSubTab === 'settings' && phpIni">
        <div class="cfg-section">基础配置</div>
        <div class="cfg-grid">
          <div class="fg"><label>内存限制 <span class="k">memory_limit</span></label><input class="input" v-model="phpIni.memory_limit" /></div>
          <div class="fg"><label>上传文件大小限制 <span class="k">upload_max_filesize</span></label><input class="input" v-model="phpIni.upload_max_filesize" /></div>
          <div class="fg"><label>POST 数据大小限制 <span class="k">post_max_size</span></label><input class="input" v-model="phpIni.post_max_size" /></div>
          <div class="fg"><label>最大执行时间(秒) <span class="k">max_execution_time</span></label><input class="input" type="number" v-model.number="phpIni.max_execution_time" /></div>
          <div class="fg"><label>输入超时(秒) <span class="k">max_input_time</span></label><input class="input" type="number" v-model.number="phpIni.max_input_time" /></div>
          <div class="fg"><label>时区 <span class="k">date.timezone</span></label><input class="input" v-model="phpIni.date_timezone" /></div>
          <div class="fg"><label>错误报告级别 <span class="k">error_reporting</span></label><input class="input" v-model="phpIni.error_reporting" /></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">显示错误 <span class="k">display_errors</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.display_errors" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">短标签 <span class="k">short_open_tag</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.short_open_tag" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">文件上传 <span class="k">file_uploads</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.file_uploads" /><span class="toggle-slider"></span></label></div>
        </div>
        <div class="cfg-section">安全</div>
        <div class="cfg-grid">
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">允许远程文件 <span class="k">allow_url_fopen</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.allow_url_fopen" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">允许远程包含 <span class="k">allow_url_include</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.allow_url_include" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">暴露 PHP 版本 <span class="k">expose_php</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.expose_php" /><span class="toggle-slider"></span></label></div>
          <div class="fg full"><label>禁用函数 <span class="k">disable_functions</span></label><input class="input" v-model="phpIni.disable_functions" placeholder="exec,passthru,shell_exec,system" /></div>
          <div class="fg full"><label>目录限制 <span class="k">open_basedir</span></label><input class="input" v-model="phpIni.open_basedir" placeholder="留空不限制" /></div>
        </div>
        <div class="cfg-section">OPCache 缓存</div>
        <div class="cfg-grid">
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">启用 OPCache <span class="k">opcache.enable</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.opcache_enable" /><span class="toggle-slider"></span></label></div>
          <div class="fg"><label>内存消耗(MB) <span class="k">opcache.memory_consumption</span></label><input class="input" type="number" v-model.number="phpIni.opcache_memory_consumption" /></div>
          <div class="fg"><label>最大加速文件数 <span class="k">opcache.max_accelerated_files</span></label><input class="input" type="number" v-model.number="phpIni.opcache_max_accelerated_files" /></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">验证时间戳 <span class="k">opcache.validate_timestamps</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.opcache_validate_timestamps" /><span class="toggle-slider"></span></label></div>
          <div class="fg"><label>重验证间隔(秒) <span class="k">opcache.revalidate_freq</span></label><input class="input" type="number" v-model.number="phpIni.opcache_revalidate_freq" /></div>
        </div>
        <div class="cfg-section">输出与编码</div>
        <div class="cfg-grid">
          <div class="fg"><label>输出缓冲 <span class="k">output_buffering</span></label><input class="input" v-model="phpIni.output_buffering" /></div>
          <div class="fg"><label>默认字符集 <span class="k">default_charset</span></label><input class="input" v-model="phpIni.default_charset" /></div>
          <div class="fg"><label>最大上传文件数 <span class="k">max_file_uploads</span></label><input class="input" type="number" v-model.number="phpIni.max_file_uploads" /></div>
          <div class="fg"><label>Socket 超时(秒) <span class="k">default_socket_timeout</span></label><input class="input" type="number" v-model.number="phpIni.default_socket_timeout" /></div>
        </div>
        <div class="cfg-section">Session 会话</div>
        <div class="cfg-grid">
          <div class="fg"><label>存储方式 <span class="k">session.save_handler</span></label><SelectMenu v-model="phpIni.session_save_handler" :options="phpSessionHandlerOptions" full-width trigger-class="input" /></div>
          <div class="fg"><label>存储路径 <span class="k">session.save_path</span></label><input class="input" v-model="phpIni.session_save_path" placeholder="默认系统临时目录" /></div>
          <div class="fg"><label>Session 名称 <span class="k">session.name</span></label><input class="input" v-model="phpIni.session_name" /></div>
          <div class="fg"><label>GC 最大存活时间(秒) <span class="k">session.gc_maxlifetime</span></label><input class="input" type="number" v-model.number="phpIni.session_gc_maxlifetime" /></div>
          <div class="fg"><label>Cookie 有效期(秒) <span class="k">session.cookie_lifetime</span></label><input class="input" type="number" v-model.number="phpIni.session_cookie_lifetime" /></div>
          <div class="fg"><label>Cookie SameSite <span class="k">session.cookie_samesite</span></label><SelectMenu v-model="phpIni.session_cookie_samesite" :options="phpSameSiteOptions" full-width trigger-class="input" /></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">使用 Cookie <span class="k">session.use_cookies</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.session_use_cookies" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">仅使用 Cookie <span class="k">session.use_only_cookies</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.session_use_only_cookies" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">严格模式 <span class="k">session.use_strict_mode</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.session_use_strict_mode" /><span class="toggle-slider"></span></label></div>
          <div class="fg-toggle"><span class="text-[12px] font-medium" style="color:var(--text-secondary)">Cookie HttpOnly <span class="k">session.cookie_httponly</span></span><label class="toggle-wrap"><input type="checkbox" v-model="phpIni.session_cookie_httponly" /><span class="toggle-slider"></span></label></div>
        </div>
      </div>
    </div>


    <!-- ==================== Hosts ==================== -->
    <div v-if="activeTab === 'hosts'" class="tab-card p-6">
      <div class="flex items-center justify-between mb-2">
        <div class="text-sm font-medium text-content-secondary">系统 Hosts 文件</div>
        <button class="btn btn-secondary btn-sm" @click="openHostsExternally">系统编辑器打开</button>
      </div>
      <div class="text-xs text-content-muted mb-3 break-all">{{ hostsPath || '加载中...' }}</div>
      <textarea
        class="input font-mono text-xs leading-5 w-full min-h-[420px]"
        style="resize: vertical"
        v-model="hostsText"
        placeholder="# 在这里编辑 hosts 内容"
      ></textarea>
    </div>

    <!-- ==================== 全局环境 ==================== -->
    <!-- v-show 而非 v-if：进入 /config 即挂载 GlobalEnv 在后台拉数据，
         切到该 tab 时无网络等待。 -->
    <div v-show="activeTab === 'env'" class="tab-card p-6">
      <GlobalEnv />
    </div>

    <!-- Unified save bar （env tab 自带操作按钮，不需要统一保存条） -->
    <div v-if="activeTab !== 'env'" class="save-bar">
      <button v-if="(activeTab === 'nginx' && nginx) || (activeTab === 'mysql' && mysql) || (activeTab === 'redis' && redis) || (activeTab === 'php' && phpSubTab === 'settings' && phpIni) || activeTab === 'hosts'"
              class="btn btn-success btn-sm" :disabled="busy || (activeTab === 'hosts' && !hostsDirty())"
              @click="activeTab === 'nginx' ? saveNginx() : activeTab === 'mysql' ? saveMysql() : activeTab === 'redis' ? saveRedis() : activeTab === 'php' ? savePhpIni() : saveHosts()">
        {{ busy ? "保存中..." : "保存配置" }}
      </button>
      <button class="btn btn-secondary btn-sm" @click="openConfigFile">打开配置文件</button>
      <button v-if="activeTab === 'nginx' || activeTab === 'mysql'" class="btn btn-secondary btn-sm" @click="viewLog">查看日志</button>
      <span v-if="activeTab === 'hosts' && hostsDirty()" class="saved-msg" style="color: var(--color-warn, #f59e0b)">hosts 有未保存修改</span>
      <span v-else-if="saved" class="saved-msg">已保存，重启服务后生效</span>
    </div>

    <!-- Log modal -->
    <div v-if="showLog" class="modal-overlay">
      <div class="modal-content" style="width: 800px">
        <div class="flex items-center justify-between mb-3">
          <div class="text-base font-semibold">错误日志</div>
          <button class="btn btn-secondary btn-sm" @click="showLog = false">关闭</button>
        </div>
        <pre class="input font-mono text-xs whitespace-pre-wrap overflow-y-auto" style="max-height: 60vh; min-height: 200px">{{ logContent || '(日志为空)' }}</pre>
      </div>
    </div>
  </div>
</template>

