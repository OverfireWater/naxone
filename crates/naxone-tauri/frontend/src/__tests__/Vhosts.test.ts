import { describe, it, expect, vi, beforeEach } from "vitest";
import { mount } from "@vue/test-utils";
import { nextTick } from "vue";

const mocked = vi.hoisted(() => {
  const responses: Record<string, any> = {
    get_vhosts: [],
    get_php_versions: [],
    get_config: { www_root: "D:/www" },
    dir_exists: true,
  };
  const invokeMock = vi.fn(async (cmd: string, args?: any) => {
    if (cmd === "dir_exists") return responses.dir_exists;
    if (cmd === "check_port_available") return true;
    if (cmd === "create_vhost") {
      return [{ id: `${args?.req?.server_name}_80`, server_name: args?.req?.server_name, aliases: [], listen_port: 80, document_root: args?.req?.document_root, php_version: null, index_files: "", rewrite_rule: "", autoindex: false, has_ssl: false, ssl_cert: "", ssl_key: "", force_https: false, custom_directives: "", access_log: "", enabled: true, created_at: "", expires_at: "", source: "custom" }];
    }
    return responses[cmd] ?? [];
  });
  return { responses, invokeMock };
});

vi.mock("@tauri-apps/api/core", () => ({ invoke: mocked.invokeMock }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn().mockResolvedValue(null) }));
vi.mock("lucide-vue-next", () => ({
  Pencil: { template: "<span>edit</span>" },
  Trash2: { template: "<span>del</span>" },
  ExternalLink: { template: "<span>link</span>" },
  FolderOpen: { template: "<span>folder</span>" },
}));

import Vhosts from "../views/Vhosts.vue";

describe("Vhosts", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    mocked.responses.get_vhosts = [];
    mocked.responses.dir_exists = true;
  });

  it("renders empty state when no vhosts", async () => {
    const wrapper = mount(Vhosts);
    await nextTick();
    await nextTick();
    expect(wrapper.text()).toContain("暂无网站");
  });

  it("opens create modal on button click", async () => {
    const wrapper = mount(Vhosts);
    await nextTick();
    await nextTick();
    const createBtn = wrapper.findAll("button").find((b) => b.text().includes("新建站点"));
    expect(createBtn).toBeTruthy();
    await createBtn!.trigger("click");
    await nextTick();
    expect(wrapper.text()).toContain("新建站点");
    expect(wrapper.text()).toContain("基础配置");
  });

  it("validates empty domain name by rejecting create call", async () => {
    const wrapper = mount(Vhosts);
    await nextTick();
    await nextTick();
    const createBtn = wrapper.findAll("button").find((b) => b.text().includes("新建站点"));
    await createBtn!.trigger("click");
    await nextTick();
    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "保存");
    await saveBtn!.trigger("click");
    expect(mocked.invokeMock).not.toHaveBeenCalledWith("create_vhost", expect.anything());
  });

  it("validates empty document root by rejecting create call", async () => {
    const wrapper = mount(Vhosts);
    await nextTick();
    await nextTick();
    const createBtn = wrapper.findAll("button").find((b) => b.text().includes("新建站点"));
    await createBtn!.trigger("click");
    await nextTick();

    const domainInput = wrapper.find("input[placeholder='example.test']");
    await domainInput.setValue("test.nm");
    const docInput = wrapper.find("input[placeholder='D:/www/mysite']");
    await docInput.setValue("");

    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "保存");
    await saveBtn!.trigger("click");
    expect(mocked.invokeMock).not.toHaveBeenCalledWith("create_vhost", expect.anything());
  });

  it("detects duplicate vhost", async () => {
    mocked.responses.get_vhosts = [{ id: "test.nm_80", server_name: "test.nm", aliases: [], listen_port: 80, document_root: "D:/test", php_version: null, index_files: "", rewrite_rule: "", autoindex: false, has_ssl: false, ssl_cert: "", ssl_key: "", force_https: false, custom_directives: "", access_log: "", enabled: true, created_at: "", expires_at: "", source: "custom" }];

    const wrapper = mount(Vhosts);
    await nextTick();
    await nextTick();

    const createBtn = wrapper.findAll("button").find((b) => b.text().includes("新建站点"));
    await createBtn!.trigger("click");
    await nextTick();

    const domainInput = wrapper.find("input[placeholder='example.test']");
    await domainInput.setValue("test.nm");

    const saveBtn = wrapper.findAll("button").find((b) => b.text() === "保存");
    await saveBtn!.trigger("click");

    expect(mocked.invokeMock).not.toHaveBeenCalledWith("create_vhost", expect.anything());
  });
});


