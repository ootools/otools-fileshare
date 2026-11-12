import './api/otoolsBridge.js';
import { createApp } from 'vue';
import ElementPlus, { ElMessage } from 'element-plus';
import 'element-plus/dist/index.css';
import App from './App.vue';
import router from './router';
import i18n from './i18n';

const app = createApp(App);
app.config.errorHandler = (error) => ElMessage.error(error?.message || String(error));
app.use(router);
app.use(i18n);
app.use(ElementPlus);
app.mount('#app');
