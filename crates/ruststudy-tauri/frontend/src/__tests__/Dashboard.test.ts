import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

const mockServices = [
  { id: "nginx-1.15.11", kind: "nginx", display_name: "Nginx 1.15.11", version: "1.15.11", variant: null, port: 80, status: { state: "Stopped" }, install_path: "D:\\test" },
  { id: "mysql-5.7.26", kind: "mysql", display_name: "MySQL 5.7.26", version: "5.7.26", variant: null, port: 3306, status: { state: "Running", pid: 1234 }, install_path: "D:\\test" },
];

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(async (cmd: string) => {
    if (cmd === "get_services") return mockServices;
    return [];
  }),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(), toggleMaximize: vi.fn(), isMaximized: vi.fn().mockResolvedValue(false),
    close: vi.fn(), show: vi.fn(), hide: vi.fn(), setFocus: vi.fn(),
  }),
}));

vi.mock("lucide-vue-next", () => ({
  LayoutDashboard: { template: "<span />" },
  Globe: { template: "<span />" },
  Settings2: { template: "<span />" },
  Wrench: { template: "<span />" },
  ChevronLeft: { template: "<span />" },
  ChevronRight: { template: "<span />" },
  Minus: { template: "<span />" },
  Square: { template: "<span />" },
  X: { template: "<span />" },
}));

import Dashboard from "../views/Dashboard.vue";

describe("Dashboard", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("renders service groups", async () => {
    const wrapper = mount(Dashboard);
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("服务管理");
    expect(wrapper.text()).toContain("Nginx 1.15.11");
    expect(wrapper.text()).toContain("MySQL 5.7.26");
  });

  it("shows running status for MySQL", async () => {
    const wrapper = mount(Dashboard);
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("PID 1234");
  });

  it("shows stopped status for Nginx", async () => {
    const wrapper = mount(Dashboard);
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("已停止");
  });

  it("shows start button for stopped service", async () => {
    const wrapper = mount(Dashboard);
    await nextTick();
    await nextTick();
    const buttons = wrapper.findAll("button");
    const startBtn = buttons.find(b => b.text() === "启动");
    expect(startBtn).toBeTruthy();
  });

  it("shows stop button for running service with confirmation", async () => {
    const wrapper = mount(Dashboard);
    await nextTick();
    await nextTick();
    const stopBtn = wrapper.findAll("button").find(b => b.text() === "停止");
    expect(stopBtn).toBeTruthy();
    // Click stop should show confirmation
    await stopBtn!.trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("确认停止");
  });
});
