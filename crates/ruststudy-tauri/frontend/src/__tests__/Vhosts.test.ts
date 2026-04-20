import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

// Mock Tauri modules before importing component
vi.mock("@tauri-apps/api/core", () => {
  const responses: Record<string, any> = {
    get_vhosts: [],
    get_php_versions: [],
    dir_exists: true,
  };
  return {
    invoke: vi.fn(async (cmd: string, _args?: any) => {
      if (cmd === "dir_exists") return responses.dir_exists;
      if (cmd === "create_vhost") return [{ id: "test_80", server_name: "test", aliases: [], listen_port: 80, document_root: "D:/test", php_version: null, index_files: "", rewrite_rule: "", autoindex: false, has_ssl: false, ssl_cert: "", ssl_key: "", force_https: false, custom_directives: "", access_log: "", enabled: true }];
      return responses[cmd] ?? [];
    }),
  };
});

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn().mockResolvedValue(null),
}));

vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    minimize: vi.fn(), toggleMaximize: vi.fn(), isMaximized: vi.fn().mockResolvedValue(false),
    close: vi.fn(), show: vi.fn(), hide: vi.fn(), setFocus: vi.fn(),
  }),
}));

vi.mock("lucide-vue-next", () => ({
  Pencil: { template: "<span>edit</span>" },
  Trash2: { template: "<span>del</span>" },
  ExternalLink: { template: "<span>link</span>" },
  FolderOpen: { template: "<span>folder</span>" },
}));

import Vhosts from "../views/Vhosts.vue";

// Need a router mock
const mockRouter = { push: vi.fn() };

describe("Vhosts", () => {
  beforeEach(() => { vi.clearAllMocks(); });

  it("renders empty state when no vhosts", async () => {
    const wrapper = mount(Vhosts, { global: { mocks: { $router: mockRouter } } });
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("暂无虚拟主机");
  });

  it("opens create modal on button click", async () => {
    const wrapper = mount(Vhosts, { global: { mocks: { $router: mockRouter } } });
    await nextTick();
    await nextTick();
    await wrapper.find("button").trigger("click"); // "+ 新建站点"
    await nextTick();
    expect(wrapper.text()).toContain("新建站点");
    expect(wrapper.text()).toContain("基础配置");
  });

  it("validates empty domain name", async () => {
    const wrapper = mount(Vhosts, { global: { mocks: { $router: mockRouter } } });
    await nextTick();
    await nextTick();
    // Open create
    await wrapper.find("button").trigger("click");
    await nextTick();
    // Try save without domain
    const saveBtn = wrapper.findAll("button").find(b => b.text() === "保存");
    expect(saveBtn).toBeTruthy();
    await saveBtn!.trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("域名不能为空");
  });

  it("validates empty document root", async () => {
    const wrapper = mount(Vhosts, { global: { mocks: { $router: mockRouter } } });
    await nextTick();
    await nextTick();
    await wrapper.find("button").trigger("click");
    await nextTick();
    // Fill domain but clear document root
    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("test.nm"); // domain
    // document_root has default value, clear it
    const docInput = inputs.find(i => (i.element as HTMLInputElement).placeholder?.includes("mysite"));
    if (docInput) await docInput.setValue("");
    const saveBtn = wrapper.findAll("button").find(b => b.text() === "保存");
    await saveBtn!.trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("网站目录不能为空");
  });

  it("detects duplicate vhost", async () => {
    // Mock existing vhosts
    const { invoke } = await import("@tauri-apps/api/core");
    (invoke as any).mockImplementation(async (cmd: string) => {
      if (cmd === "get_vhosts") return [{ id: "test.nm_80", server_name: "test.nm", aliases: [], listen_port: 80, document_root: "D:/test", php_version: null, index_files: "", rewrite_rule: "", autoindex: false, has_ssl: false, ssl_cert: "", ssl_key: "", force_https: false, custom_directives: "", access_log: "", enabled: true }];
      if (cmd === "get_php_versions") return [];
      if (cmd === "dir_exists") return true;
      return [];
    });

    const wrapper = mount(Vhosts, { global: { mocks: { $router: mockRouter } } });
    await nextTick();
    await nextTick();
    await nextTick();

    // Open create
    const newBtn = wrapper.findAll("button").find(b => b.text().includes("新建"));
    await newBtn!.trigger("click");
    await nextTick();

    // Fill same domain:port
    const inputs = wrapper.findAll("input");
    await inputs[0].setValue("test.nm");

    const saveBtn = wrapper.findAll("button").find(b => b.text() === "保存");
    await saveBtn!.trigger("click");
    await nextTick();
    await nextTick();

    expect(wrapper.text()).toContain("已存在");
  });
});
