<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from "vue";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { toast } from "../composables/useToast";
import { onTextareaTab } from "../composables/useTextareaTab";
import SelectMenu from "../components/SelectMenu.vue";
import { Pencil, ExternalLink, FolderOpen, Trash2 } from "lucide-vue-next";

interface VhostInfo {
  id: string; server_name: string; aliases: string[]; listen_port: number;
  document_root: string; php_version: string | null; index_files: string;
  rewrite_rule: string; autoindex: boolean; has_ssl: boolean;
  ssl_cert: string; ssl_key: string; force_https: boolean;
  custom_directives: string; access_log: string; enabled: boolean;
  created_at: string; expires_at: string;
  source: string; source_name?: string | null;
}

interface PhpVersionInfo { label: string; version: string; port: number; install_path: string; }

interface FormData {
  server_name: string; aliases: string; listen_port: number; document_root: string;
  php_version: string | null; index_files: string; rewrite_rule: string;
  autoindex: boolean; ssl_cert: string | null; ssl_key: string | null;
  force_https: boolean; custom_directives: string | null; access_log: string | null;
  sync_hosts: boolean; expires_at: string;
}

const rewritePresets = [
  { label: "通用（默认）", value: "" },
  { label: "ThinkPHP", value: "location / {\n    if (!-e $request_filename) {\n        rewrite ^(.*)$ /index.php?s=$1 last;\n        break;\n    }\n}" },
  { label: "Laravel", value: "location / {\n    try_files $uri $uri/ /index.php?$query_string;\n}" },
  { label: "WordPress", value: "location / {\n    try_files $uri $uri/ /index.php?$args;\n}" },
  { label: "自定义", value: "__custom__" },
];
const boolOptions = [
  { label: "关闭", value: false },
  { label: "开启", value: true },
];
const phpVersionOptions = computed(() => [
  { label: "不使用 PHP", value: null },
  ...phpVersions.value.map((pv) => ({ label: `${pv.label} (:${pv.port})`, value: pv.version })),
]);
const rewritePresetOptions = rewritePresets.map((preset) => ({ label: preset.label, value: preset.value }));

const vhosts = ref<VhostInfo[]>([]);
const phpVersions = ref<PhpVersionInfo[]>([]);
const floatMsg = ref("");
const floatType = ref<"success" | "error">("success");
const busy = ref(false);
const searchQuery = ref("");
const showForm = ref(false);
const editingId = ref<string | null>(null);
const confirmDeleteId = ref<string | null>(null);
const modalTab = ref<"basic" | "rewrite" | "ssl">("basic");
const rewritePreset = ref("");
// WWW 根目录（用户在设置里配的），新建 vhost 时用它作 document_root 默认值
const wwwRoot = ref<string>("");
/// 是否用户手动改过 document_root（没改过时，随 server_name 输入自动拼 {www_root}/{server_name}）
const docRootTouched = ref(false);

const form = ref<FormData>({
  server_name: "", aliases: "", listen_port: 80, document_root: "",
  php_version: null, index_files: "index.php index.html", rewrite_rule: "",
  autoindex: false, ssl_cert: null, ssl_key: null, force_https: false,
  custom_directives: null, access_log: null,
  sync_hosts: true, expires_at: "",
});

function formatDate(d: string): string {
  if (!d) return "";
  // Handle both yyyy-MM-dd and yyyy/MM/dd
  return d.replace(/-/g, "/");
}

function showError(msg: string) { toast.error(String(msg)); }
function showSuccess(msg: string) { toast.success(String(msg)); }
let floatTimer: number | null = null;
function showFloat(msg: string, type: "success" | "error" = "success") {
  if (floatTimer) clearTimeout(floatTimer);
  floatMsg.value = msg; floatType.value = type;
  floatTimer = window.setTimeout(() => { floatMsg.value = ""; floatTimer = null; }, 2500);
}
async function loadVhosts() { try { vhosts.value = await invoke("get_vhosts"); } catch (e) { showError("加载失败: " + e); } }
async function loadPhpVersions() { try { phpVersions.value = await invoke("get_php_versions"); } catch (e) { showError("加载PHP版本失败: " + e); } }
async function loadWwwRoot() {
  try {
    const cfg = await invoke<{ www_root: string }>("get_config");
    wwwRoot.value = (cfg.www_root || "").replace(/\\/g, "/").replace(/\/+$/, "");
  } catch { /* 非致命：用空字符串兜底让用户自己填 */ }
}

/// 把 server_name 拼成 {www_root}/{server_name} 作为推荐 document_root
function suggestedDocRoot(serverName: string): string {
  const name = (serverName || "").trim();
  if (!wwwRoot.value) return "";
  if (!name) return wwwRoot.value + "/";
  return `${wwwRoot.value}/${name}`;
}

function openCreate() {
  editingId.value = null; modalTab.value = "basic"; rewritePreset.value = "";
  docRootTouched.value = false;
  // document_root 默认用 www_root，没配就留空让用户自己填
  const defaultRoot = wwwRoot.value ? `${wwwRoot.value}/` : "";
  form.value = { server_name: "", aliases: "", listen_port: 80, document_root: defaultRoot,
    php_version: phpVersions.value.length > 0 ? phpVersions.value[0].version : null,
    index_files: "index.php index.html", rewrite_rule: "", autoindex: false,
    ssl_cert: null, ssl_key: null, force_https: false, custom_directives: null, access_log: null,
    sync_hosts: true, expires_at: "" };
  showForm.value = true;
}

function openEdit(vh: VhostInfo) {
  editingId.value = vh.id;
  modalTab.value = "basic";
  docRootTouched.value = true;  // 编辑时不自动改
  const matched = rewritePresets.find(p => p.value === vh.rewrite_rule && p.value !== "__custom__");
  rewritePreset.value = matched ? matched.value : (vh.rewrite_rule ? "__custom__" : "");
  form.value = { server_name: vh.server_name, aliases: vh.aliases.join(" "), listen_port: vh.listen_port,
    document_root: vh.document_root, php_version: vh.php_version, index_files: vh.index_files,
    rewrite_rule: vh.rewrite_rule, autoindex: vh.autoindex, ssl_cert: vh.ssl_cert || null,
    ssl_key: vh.ssl_key || null, force_https: vh.force_https,
    custom_directives: vh.custom_directives || null, access_log: vh.access_log || null,
    sync_hosts: true, expires_at: formatDate(vh.expires_at || "") };
  showForm.value = true;
}

/// server_name 输入时如果 document_root 还没被手动改过，自动拼成 {www_root}/{server_name}
function onServerNameInput() {
  if (!editingId.value && !docRootTouched.value) {
    form.value.document_root = suggestedDocRoot(form.value.server_name);
  }
}

function onRewritePresetChange(val: string) { rewritePreset.value = val; if (val !== "__custom__") form.value.rewrite_rule = val; }
function cancelForm() { showForm.value = false; editingId.value = null; }

async function toggleVhost(vh: VhostInfo) {
  busy.value = true;
  try {
    vhosts.value = await invoke("toggle_vhost", { id: vh.id, enabled: !vh.enabled });
    showFloat(vh.enabled ? "站点已停用" : "站点已启用");
  } catch (e) { showFloat("操作失败: " + e, "error"); } finally { busy.value = false; }
}

async function openSite(vh: VhostInfo) {
  const protocol = vh.has_ssl ? "https" : "http";
  const port = vh.listen_port === 80 || (vh.has_ssl && vh.listen_port === 443) ? "" : `:${vh.listen_port}`;
  await invoke("open_in_browser", { url: `${protocol}://${vh.server_name}${port}` });
}

async function openDir(vh: VhostInfo) {
  await invoke("open_folder", { path: vh.document_root });
}

async function saveVhost() {
  if (!form.value.server_name.trim()) { showError("域名不能为空"); return; }
  if (!form.value.document_root.trim()) { showError("网站目录不能为空"); return; }
  // Check duplicate domain:port
  const newId = `${form.value.server_name}_${form.value.listen_port}`;
  const dup = vhosts.value.find(v => v.id === newId && v.id !== editingId.value);
  if (dup) { showError(`站点 ${form.value.server_name}:${form.value.listen_port} 已存在`); return; }
  busy.value = true;
  try {
    const isEdit = !!editingId.value;
    if (isEdit) vhosts.value = await invoke("update_vhost", { id: editingId.value, req: form.value });
    else vhosts.value = await invoke("create_vhost", { req: form.value });
    showForm.value = false; editingId.value = null;
    showSuccess(isEdit ? "站点更新成功，已自动 reload Web 服务器" : "站点创建成功，已自动 reload Web 服务器");
  } catch (e) { showError("保存失败: " + e); } finally { busy.value = false; }
}

async function deleteVhost(id: string) {
  busy.value = true; confirmDeleteId.value = null;
  try { vhosts.value = await invoke("delete_vhost", { id }); showSuccess("站点已删除，已自动 reload Web 服务器"); } catch (e) { showError("删除失败: " + e); } finally { busy.value = false; }
}

async function browseFolder() { const s = await open({ directory: true, multiple: false }); if (s) { form.value.document_root = s as string; docRootTouched.value = true; } }
async function browseFile(field: "ssl_cert" | "ssl_key") { const s = await open({ directory: false, multiple: false, filters: [{ name: "证书", extensions: ["pem","crt","key","cer"] }] }); if (s) form.value[field] = s as string; }

const sslBusy = ref(false);
async function autoGenCert() {
  if (sslBusy.value) return;
  if (!form.value.server_name.trim()) {
    showError("请先在基础配置里填 server_name（域名）");
    modalTab.value = "basic";
    return;
  }
  sslBusy.value = true;
  try {
    const aliases = form.value.aliases.split(/\s+/).filter(Boolean);
    const res = await invoke<{ cert_path: string; key_path: string }>(
      "generate_self_signed_cert",
      { serverName: form.value.server_name, aliases }
    );
    form.value.ssl_cert = res.cert_path;
    form.value.ssl_key = res.key_path;
    showSuccess("自签证书生成成功，保存站点即生效");
  } catch (e) {
    showError("生成失败: " + e);
  } finally {
    sslBusy.value = false;
  }
}

const filteredVhosts = computed(() => {
  if (!searchQuery.value.trim()) return vhosts.value;
  const q = searchQuery.value.toLowerCase();
  return vhosts.value.filter(v =>
    v.server_name.toLowerCase().includes(q) ||
    v.document_root.toLowerCase().includes(q) ||
    (v.php_version || "").toLowerCase().includes(q) ||
    v.aliases.some(a => a.toLowerCase().includes(q))
  );
});

// Grouped view: 自建/独立站点优先（用户自己创建的更常用），PHPStudy 站点（导入的）放下面
interface VhostGroup { key: string; label: string; items: VhostInfo[]; }
const groupedVhosts = computed<VhostGroup[]>(() => {
  const phpstudy = filteredVhosts.value.filter(v => v.source === "phpstudy");
  const custom = filteredVhosts.value.filter(v => v.source !== "phpstudy");
  const out: VhostGroup[] = [];
  if (custom.length) out.push({ key: "custom", label: "自建站点", items: custom });
  if (phpstudy.length) out.push({ key: "phpstudy", label: "PHPStudy 站点", items: phpstudy });
  return out;
});

/// 把 vhost 存的 php_version（"php855nts" 这种内部 id）反查成 { v: "8.5.5", variant: "nts" }。
/// 优先用后端 phpVersions 里的 label（权威格式），找不到再 fallback 解析。
function phpDisplay(raw: string | null): { v: string; variant: string } | null {
  if (!raw) return null;
  const found = phpVersions.value.find((p) => p.version === raw);
  if (found) {
    // label 形如 "PHP 8.5.5" 或 "PHP 8.5.5 (nts)"
    const m = found.label.match(/^PHP\s+(\S+)(?:\s+\((\S+)\))?/);
    if (m) return { v: m[1], variant: (m[2] || "").toLowerCase() };
  }
  // Fallback：vhost 引用了已删除/外部的 PHP 版本，直接按 "php{digits}{variant}" 解
  const m = raw.match(/^php(\d+)(nts|ts)?$/i);
  if (m) {
    const d = m[1];
    const v =
      d.length === 3
        ? `${d[0]}.${d[1]}.${d[2]}`
        : d.length >= 4
          ? `${d[0]}.${d[1]}.${d.slice(2)}`
          : d;
    return { v, variant: (m[2] || "").toLowerCase() };
  }
  return { v: raw, variant: "" };
}

function sourceBadge(v: VhostInfo): { text: string; color: string } | null {
  if (v.source === "phpstudy") return { text: "PS", color: "#8b5cf6" };
  if (v.source === "standalone") return { text: "独立", color: "#06b6d4" };
  return null; // custom: no badge, it's the default
}

function isExpired(vh: VhostInfo): boolean {
  if (!vh.expires_at) return false;
  const today = new Date().toISOString().slice(0, 10);
  return vh.expires_at.replace(/\//g, "-") <= today;
}

let expiryTimer: number | null = null;
async function checkExpired() {
  try { vhosts.value = await invoke("check_expired_vhosts"); } catch {}
}

onMounted(() => {
  loadVhosts(); loadPhpVersions(); loadWwwRoot();
  expiryTimer = window.setInterval(checkExpired, 60000); // Check every 60s
});
onUnmounted(() => { if (expiryTimer) clearInterval(expiryTimer); });
</script>

<template>
  <div class="max-w-[960px]">
    <!-- Header -->
    <div class="flex items-center justify-end mb-3">
      <div class="flex items-center gap-2">
        <input class="input" style="width: 200px" v-model="searchQuery" placeholder="搜索站点..." />
        <button class="btn btn-success btn-sm" @click="openCreate" :disabled="busy">+ 新建站点</button>
      </div>
    </div>

    <!-- Floating toast (per-row position, kept for expires-soon etc.) -->
    <div v-if="floatMsg" class="float-toast" :class="floatType">{{ floatMsg }}</div>

    <!-- ==================== Modal ==================== -->
    <div v-if="showForm" class="modal-overlay">
      <div class="modal-content">
        <div class="flex items-center justify-between mb-3">
          <div class="text-base font-semibold">{{ editingId ? "编辑站点" : "新建站点" }}</div>
          <div class="modal-actions !mt-0">
            <button class="btn btn-secondary btn-sm" @click="cancelForm" :disabled="busy">取消</button>
            <button class="btn btn-success btn-sm" @click="saveVhost" :disabled="busy">{{ busy ? "保存中..." : "保存" }}</button>
          </div>
        </div>

        <!-- Modal tabs -->
        <div class="flex gap-0 border-b mb-3" style="border-color: var(--border-color)">
          <button v-for="t in [{k:'basic',l:'基础配置'},{k:'rewrite',l:'伪静态'},{k:'ssl',l:'SSL 与高级'}]" :key="t.k"
            class="modal-tab" :class="{ active: modalTab === t.k }" @click="modalTab = t.k as any">{{ t.l }}</button>
        </div>

        <!-- Tab: 基础配置 -->
        <div v-show="modalTab === 'basic'" class="grid grid-cols-2 gap-3">
          <div class="fg"><label>域名 <span style="color:var(--color-danger)">*</span></label><input class="input" v-model="form.server_name" @input="onServerNameInput" placeholder="example.test" /></div>
          <div class="fg"><label>端口</label><input class="input" type="number" v-model.number="form.listen_port" min="1" max="65535" /></div>
          <div class="fg full"><label>域名别名 <span style="color:var(--text-muted);font-weight:400">多个用空格分隔</span></label><input class="input" v-model="form.aliases" placeholder="www.example.test" /></div>
          <div class="fg full"><label>网站目录 <span style="color:var(--color-danger)">*</span></label><div class="flex gap-1.5"><input class="input flex-1" v-model="form.document_root" @input="docRootTouched = true" :placeholder="wwwRoot ? wwwRoot + '/mysite' : '/path/to/site'" /><button class="btn btn-secondary btn-sm" @click="browseFolder">浏览</button></div></div>
          <div class="fg"><label>PHP 版本</label><SelectMenu v-model="form.php_version" :options="phpVersionOptions" full-width trigger-class="input" /></div>
          <div class="fg"><label>默认首页</label><input class="input" v-model="form.index_files" placeholder="index.php index.html" /></div>
          <div class="fg"><label>到期日期 <span style="color:var(--text-muted);font-weight:400">留空永不过期</span></label><input class="input" type="date" v-model="form.expires_at" /></div>
          <div class="fg-toggle"><span class="text-[13px] font-medium" style="color:var(--text-secondary)">同步 Hosts</span><label class="toggle-wrap"><input type="checkbox" v-model="form.sync_hosts" /><span class="toggle-slider"></span></label></div>
        </div>

        <!-- Tab: 伪静态 -->
        <div v-show="modalTab === 'rewrite'" class="grid grid-cols-2 gap-3">
          <div class="fg"><label>伪静态预设</label><SelectMenu :model-value="rewritePreset" :options="rewritePresetOptions" full-width trigger-class="input" @update:modelValue="onRewritePresetChange(String($event))" /></div>
          <div class="fg"><label>目录浏览</label><SelectMenu v-model="form.autoindex" :options="boolOptions" full-width trigger-class="input" /></div>
          <div class="fg full"><label>伪静态规则 <span style="color:var(--text-muted);font-weight:400">选择预设自动填充，可手动修改</span></label><textarea class="input resize-y font-mono text-[13px] min-h-[180px]" v-model="form.rewrite_rule" rows="5" spellcheck="false" @keydown="onTextareaTab" placeholder="try_files $uri $uri/ /index.php?$query_string;"></textarea></div>
        </div>

        <!-- Tab: SSL 与高级 -->
        <div v-show="modalTab === 'ssl'" class="grid grid-cols-2 gap-3">
          <div class="fg full">
            <div class="flex items-center justify-between mb-1">
              <label class="!mb-0">SSL 证书</label>
              <button class="btn btn-primary btn-sm" :disabled="sslBusy" @click="autoGenCert">
                {{ sslBusy ? '生成中...' : '🔒 一键生成自签证书' }}
              </button>
            </div>
            <div class="text-[13px]" style="color: var(--text-muted)">
              本地开发 HTTPS 用。浏览器首次访问会提示"不安全"，手动继续即可；或用 mkcert 把本地 CA 装进系统信任。
            </div>
          </div>
          <div class="fg"><label>证书路径</label><div class="flex gap-1.5"><input class="input flex-1" v-model="form.ssl_cert" placeholder="*.pem / *.crt" /><button class="btn btn-secondary btn-sm" @click="browseFile('ssl_cert')">浏览</button></div></div>
          <div class="fg"><label>密钥路径</label><div class="flex gap-1.5"><input class="input flex-1" v-model="form.ssl_key" placeholder="*.key" /><button class="btn btn-secondary btn-sm" @click="browseFile('ssl_key')">浏览</button></div></div>
          <div class="fg"><label>强制 HTTPS</label><SelectMenu v-model="form.force_https" :options="boolOptions" full-width trigger-class="input" /></div>
          <div class="fg"><label>日志路径</label><input class="input" v-model="form.access_log" placeholder="留空使用默认" /></div>
          <div class="fg full"><label>自定义 Nginx 指令</label><textarea class="input resize-y font-mono text-[13px] min-h-[140px]" v-model="form.custom_directives" rows="4" spellcheck="false" @keydown="onTextareaTab" placeholder="proxy_pass http://127.0.0.1:3000;"></textarea></div>
        </div>
      </div>
    </div>

    <!-- ==================== List ==================== -->
    <div v-if="filteredVhosts.length === 0 && !showForm" class="text-center py-16 text-content-secondary">
      <p>暂无网站</p>
      <p class="mt-2 text-[16px] text-content-muted">点击上方按钮创建第一个站点</p>
    </div>

    <template v-for="g in groupedVhosts" :key="g.key">
    <div class="flex items-center gap-2 px-1 mt-3 mb-1.5 text-[13px] font-semibold uppercase tracking-widest"
         style="color: var(--text-muted)">
      <span>{{ g.label }}</span>
      <span class="px-1.5 py-px rounded text-[13px] font-mono" style="background: var(--bg-tertiary)">{{ g.items.length }}</span>
    </div>
    <div class="list-card mb-2">
      <!-- Header -->
      <div class="flex items-center px-5 py-2.5 text-[13px] font-semibold uppercase tracking-widest"
           style="color: var(--text-muted)">
        <span class="w-[200px]">域名</span>
        <span class="w-14">端口</span>
        <span class="flex-1 min-w-0">网站目录</span>
        <span class="w-20 text-center">PHP</span>
        <span class="w-20 text-center">到期</span>
        <span class="w-14 text-center">状态</span>
        <span class="w-40 text-right">操作</span>
      </div>
      <!-- Rows -->
      <div v-for="vh in g.items" :key="vh.id" class="group list-row gap-0">
        <div class="w-[200px] shrink-0 min-w-0">
          <div class="flex items-center gap-1.5">
            <span class="text-[16px] font-medium truncate">{{ vh.server_name }}</span>
            <span v-if="vh.has_ssl" class="text-[13px] px-1.5 py-px rounded font-medium shrink-0"
                  style="background: rgba(34,197,94,0.15); color: var(--color-success-light)">SSL</span>
            <span v-if="sourceBadge(vh)" class="text-[13px] px-1.5 py-px rounded font-medium shrink-0"
                  :style="{ background: `${sourceBadge(vh)!.color}22`, color: sourceBadge(vh)!.color }">{{ sourceBadge(vh)!.text }}</span>
          </div>
          <div v-if="vh.aliases.length > 0" class="text-[13px] mt-0.5 truncate" style="color: var(--text-muted)">{{ vh.aliases.join(", ") }}</div>
        </div>
        <div class="w-14 shrink-0 text-[16px] font-mono" style="color: var(--text-muted)">{{ vh.listen_port }}</div>
        <div class="flex-1 min-w-0 text-[16px] truncate pr-3" style="color: var(--text-secondary)" :title="vh.document_root">{{ vh.document_root }}</div>
        <div class="w-20 shrink-0 text-center">
          <template v-if="phpDisplay(vh.php_version)">
            <span class="text-[13px] font-mono" style="color: var(--text-secondary)">{{ phpDisplay(vh.php_version)!.v }}</span>
            <span v-if="phpDisplay(vh.php_version)!.variant" class="text-[13px] ml-0.5" style="color: var(--text-muted)">{{ phpDisplay(vh.php_version)!.variant }}</span>
          </template>
          <span v-else class="text-[13px]" style="color: var(--text-muted)">-</span>
        </div>
        <div class="w-20 shrink-0 text-center text-[13px]" :style="{ color: isExpired(vh) ? 'var(--color-danger)' : 'var(--text-muted)' }">{{ vh.expires_at ? (isExpired(vh) ? '已过期' : formatDate(vh.expires_at)) : '永久' }}</div>
        <div class="w-14 shrink-0 flex justify-center">
          <label class="toggle-wrap" @click.prevent="toggleVhost(vh)">
            <input type="checkbox" :checked="vh.enabled" />
            <span class="toggle-slider"></span>
          </label>
        </div>
        <div class="w-40 shrink-0 flex items-center gap-1 justify-end">
          <template v-if="confirmDeleteId === vh.id">
            <button class="btn btn-danger btn-sm" @click="deleteVhost(vh.id)" :disabled="busy">确认</button>
            <button class="btn btn-secondary btn-sm" @click="confirmDeleteId = null">取消</button>
          </template>
          <template v-else>
            <button class="btn btn-secondary btn-sm !px-2 transition-opacity" @click="openEdit(vh)" :disabled="busy" title="编辑"><Pencil :size="14" /></button>
            <button class="btn btn-secondary btn-sm !px-2 transition-opacity" @click="openSite(vh); showFloat('已在浏览器打开')" title="在浏览器打开"><ExternalLink :size="14" /></button>
            <button class="btn btn-secondary btn-sm !px-2 transition-opacity" @click="openDir(vh); showFloat('已打开目录')" title="打开目录"><FolderOpen :size="14" /></button>
            <button class="btn btn-secondary btn-sm !px-2 transition-opacity hover:!text-red-400" @click="confirmDeleteId = vh.id" :disabled="busy" title="删除"><Trash2 :size="14" /></button>
          </template>
        </div>
      </div>
    </div>
    </template>
  </div>
</template>
