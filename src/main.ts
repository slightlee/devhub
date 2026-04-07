// 应用入口：挂载根组件并加载全局样式。
import { createApp } from "vue";
import App from "./App.vue";
import "./assets/styles.css";

createApp(App).mount("#app");
