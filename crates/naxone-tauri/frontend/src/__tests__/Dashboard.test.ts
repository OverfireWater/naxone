import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

const mocked = vi.hoisted(() => {
  const services = [
    { id: "nginx-1.15.11", kind: "nginx", display_name: "Nginx 1.15.11", version: "1.15.11", variant: null, port: 80, status: { state: "Stopped" }, install_path: "D:/test", origin: "manual" },
    { id: "mysql-5.7.26", kind: "mysql", display_name: "MySQL 5.7.26", version: "5.7.26", variant: null, port: 3306, status: { state: "Running", pid: 1234 }, install_path: "D:/test", origin: "manual" },
    { id: "php-8.2.0", kind: "php", display_name: "PHP-CGI 8.2.0", version: "8.2.0", variant: "nts", port: 9000, status: { state: "Stopped" }, install_path: "D:/test", origin: "manual" },
  ];
  const invokeMock = vi.fn(async (cmd: string, args?: any) => {
    if (cmd === "get_services") return services;
    if (cmd === "get_startup_errors") return [];
    if (cmd === "get_logs") return [];
    if (cmd === "get_global_php_version") return { version: null, bin_dir: "", path_registered: false, conflicts: [] };
    if (cmd === "get_app_stats") return { pid: 9999, memory_mb: 88, uptime_secs: 12, is_dev: true };
    if (cmd === "check_for_updates") return { available: false, latest_version: "", current_version: "", release_url: "", release_notes: "", download_url: null };
    if (cmd === "start_service") return services;
    if (cmd === "stop_service") return services[0];
    if (cmd === "restart_service") return services[0];
    if (cmd === "start_all") return services;
    if (cmd === "stop_all") return services;
    if (cmd === "open_in_browser") return null;
    if (cmd === "open_system_env_editor") return null;
    if (cmd === "fix_global_php_conflicts") return { version: null, bin_dir: "", path_registered: false, conflicts: [] };
    if (cmd === "set_global_php_version") return { version: args?.version ?? "8.2.0", bin_dir: "", path_registered: true, conflicts: [] };
    return [];
  });
  return { invokeMock };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocked.invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ confirm: vi.fn().mockResolvedValue(true) }));
vi.mock("@tauri-apps/api/event", () => ({ listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("vue-router", () => ({ useRouter: () => ({ push: vi.fn() }) }));

vi.mock("lucide-vue-next", () => ({
  AlertCircle: { template: "<span />" },
  AlertTriangle: { template: "<span />" },
  CheckCircle2: { template: "<span />" },
  Info: { template: "<span />" },
  Bug: { template: "<span />" },
  ChevronRight: { template: "<span />" },
  Store: { template: "<span />" },
  Settings2: { template: "<span />" },
}));

vi.mock("../composables/useAppInfo", () => ({ APP_NAME: "NaxOne (Development)" }));

import Dashboard from "../views/Dashboard.vue";

describe("Dashboard", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("renders main controls and service groups", async () => {
    const wrapper = mount(Dashboard, { global: { stubs: { LogDrawer: true } } });
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("全部启动");
    expect(wrapper.text()).toContain("全部停止");
    expect(wrapper.text()).toContain("Nginx");
    expect(wrapper.text()).toContain("MySQL");
  });

  it("shows running status for running service group", async () => {
    const wrapper = mount(Dashboard, { global: { stubs: { LogDrawer: true } } });
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("运行中");
  });

  it("shows stopped status for stopped group", async () => {
    const wrapper = mount(Dashboard, { global: { stubs: { LogDrawer: true } } });
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("已停止");
  });

  it("renders php engine section when php exists", async () => {
    const wrapper = mount(Dashboard, { global: { stubs: { LogDrawer: true } } });
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("PHP 引擎");
  });

  it("calls start_all command on bulk start click", async () => {
    const wrapper = mount(Dashboard, { global: { stubs: { LogDrawer: true } } });
    await nextTick();
    await nextTick();
    const startBtn = wrapper.findAll("button").find((b) => b.text() === "全部启动");
    expect(startBtn).toBeTruthy();
    await startBtn!.trigger("click");
    expect(mocked.invokeMock).toHaveBeenCalledWith("start_all", undefined);
  });
});


