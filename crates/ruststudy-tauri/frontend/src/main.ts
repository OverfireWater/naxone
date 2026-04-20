import "./assets/global.css";
import { createApp } from "vue";
import { createRouter, createWebHistory } from "vue-router";
import App from "./App.vue";
import Dashboard from "./views/Dashboard.vue";
import Vhosts from "./views/Vhosts.vue";
import ServiceConfig from "./views/ServiceConfig.vue";
import Settings from "./views/Settings.vue";
import SoftwareStore from "./views/SoftwareStore.vue";

const router = createRouter({
  history: createWebHistory(),
  routes: [
    { path: "/", component: Dashboard },
    { path: "/vhosts", component: Vhosts },
    { path: "/store", component: SoftwareStore },
    { path: "/config", component: ServiceConfig },
    { path: "/settings", component: Settings },
  ],
});

const app = createApp(App);
app.use(router);
app.mount("#app");
