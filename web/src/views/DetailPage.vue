<template>
  <div class="detail-space">
    <el-card class="box-card">
      <template #header>
        <div class="card-header">
          <div class="header-flex">
            <div>{{ $t("detail.sharing") }}</div>
            <div>
              <el-button type="danger" @click="stopServer" plain
                >{{ $t("detail.stopShare") }}</el-button
              >
            </div>
          </div>
        </div>
      </template>
      <div class="share-flex">
        <div class="share-url">
          <el-tooltip effect="light" placement="top" :content="settingForm.url">
            <span class="url-text"
              >{{ $t("detail.shareLink") }}{{ settingForm.url }}</span
            >
          </el-tooltip>
        </div>
        <div class="share-action">
          <el-popover placement="left" :width="125" trigger="hover">
            <template #reference>
              <el-button
                type="default"
                :title="$t('detail.copyLinkTip')"
                @click="handleClipboard(settingForm.url, $event)"
              >
                <el-icon>
                  <Link />
                </el-icon>
                &nbsp;{{ $t("detail.copyLink") }}
              </el-button>
            </template>
            <qrcode-vue :value="settingForm.url"></qrcode-vue>
          </el-popover>
        </div>
        <div class="share-action">
          <el-tooltip
            effect="dark"
            :content="$t('detail.switchIpProtocol')"
            placement="top-start"
          >
            <el-button
              type="default"
              :title="$t('detail.switchIpProtocol')"
              @click="changeIpFamily()"
            >
              <el-icon>
                <Sort />
              </el-icon>
              &nbsp;{{ $t("detail.switchTo")
              }}{{ ipFamily === "ipv6" ? "ipv4" : "ipv6" }}
            </el-button>
          </el-tooltip>
        </div>
        <div class="share-action">
          <el-tooltip
            effect="dark"
            :content="$t('detail.switchNetInterface')"
            placement="top-start"
          >
            <el-button
              type="default"
              :title="$t('detail.switchNetInterface')"
              :disabled="netInterfaceNames.length <= 1"
              @click="changeNetInterface()"
            >
              <el-icon>
                <Sort />
              </el-icon>
              &nbsp;{{ $t("detail.switchNetInterface") }}
            </el-button>
          </el-tooltip>
        </div>
      </div>
    </el-card>

    <el-card class="box-card">
      <template #header>
        <el-dialog
          v-model="dialogFormVisible"
          :title="$t('detail.shareText')"
        >
          <el-input
            type="textarea"
            :rows="2"
            :autosize="{ minRows: 2, maxRows: 4 }"
            :placeholder="$t('detail.inputContent')"
            v-model="form.text"
          >
          </el-input>
          <template #footer>
            <el-button type="primary" @click="formSubmit">{{
              $t("detail.submit")
            }}</el-button>
          </template>
        </el-dialog>

        <div class="card-header">
          <div class="list-header-flex">
            <div class="list-title">{{ $t("detail.shareList") }}</div>
            <div class="list-action">
              <el-button
                @click="dialogFormVisible = true"
                type="default"
                :title="$t('detail.shareText')"
              >
                <el-icon>
                  <Message />
                </el-icon>
                &nbsp;{{ $t("detail.shareTextBtn") }}
              </el-button>
            </div>
            <div class="list-action">
              <el-popconfirm
                @confirm="removeFileAll()"
                :title="$t('detail.clearListConfirm')"
              >
                <template #reference>
                  <el-button type="default" :title="$t('detail.clearList')">
                    <el-icon><Delete /></el-icon>
                    &nbsp;{{ $t("detail.clearList") }}
                  </el-button>
                </template>
              </el-popconfirm>
            </div>
          </div>
        </div>
      </template>

      <div class="upload-box native-drop-zone">
        <p>{{ $t('detail.dropHint') }}</p>
        <div>
          <el-button @click="pickPaths(false)">{{ $t('detail.addFiles') }}</el-button>
          <el-button @click="pickPaths(true)">{{ $t('detail.addFolder') }}</el-button>
        </div>
      </div>
      <div v-if="transfers.length" class="transfer-list">
        <div v-for="transfer in transfers" :key="transfer.name">{{ transfer.name }} · {{ Math.round(transfer.bytes / 1024) }} KB <span v-if="transfer.complete">✓</span></div>
      </div>

      <div v-for="file in files" :key="file" class="text item file-item">
        <el-row class="row-bg" justify="space-between">
          <el-col :span="5">
            <el-tooltip effect="light" placement="top">
              <template #content>{{
                $t("detail.sharedBy", { name: file.username })
              }}</template>
              <span class="username">{{ file.username }}</span>
            </el-tooltip>
          </el-col>
          <el-col :span="13">
            <el-tooltip
              v-if="file.type === 'text'"
              effect="light"
              placement="top"
            >
              <template #content>{{ file.intro }}</template>
              <span>{{ file.name }}</span>
            </el-tooltip>
            <span v-if="['directory', 'file'].includes(file.type)">{{
              file.name
            }}</span>
          </el-col>
          <el-col :span="6">
            <!-- 复制链接 -->
            <el-popover placement="left" :width="125" trigger="hover">
              <template #reference>
                <el-button
                  :style="
                    ['directory', 'file'].includes(file.type)
                      ? ''
                      : 'visibility:hidden;'
                  "
                  type="default"
                  size="small"
                  :title="$t('detail.copyLinkTip')"
                  @click="handleFileUrlCopy(file, $event)"
                >
                  <el-icon><Link /></el-icon>
                </el-button>
              </template>
              <qrcode-vue :value="getFileUrl(file)"></qrcode-vue>
            </el-popover>
            <!-- 复制文本 -->
            <el-button
              v-if="['text'].includes(file.type)"
              type="default"
              size="small"
              :title="$t('detail.copyTextTip')"
              @click="handleClipboard(file.content, $event)"
            >
              <el-icon><DocumentCopy /></el-icon>
            </el-button>
            <!-- 打开文件 -->
            <el-button
              v-if="['directory', 'file'].includes(file.type)"
              type="default"
              size="small"
              :title="$t('detail.openFileDir')"
              @click="openFile(file.name, $event)"
            >
              <el-icon><FolderOpened /></el-icon>
            </el-button>
            <!-- 删除 -->
            <el-button
              type="default"
              size="small"
              @click="() => removeFile(file)"
            >
              <el-icon><Delete /></el-icon>
            </el-button>
          </el-col>
        </el-row>
      </div>

      <el-alert
        v-if="files.length === 0"
        :title="$t('common.none')"
        :closable="false"
        type="info"
        center
      >
      </el-alert>
    </el-card>
  </div>
</template>

<script>
import Clipboard from "clipboard";
import { ElMessage } from "element-plus";
import QrcodeVue from "qrcode.vue";
import {
  Delete,
  DocumentCopy,
  FolderOpened,
  Link,
  Message,
  Sort,
} from "@element-plus/icons-vue";
import { store } from "@/store";
import { successMessage } from "@/utils/message";
import { t } from "@/i18n";
import { pathName } from '@/api/launch';

let api = window.api;

// 查找最近的按钮元素
const findButtonElement = (element) => {
  while (element && !element.matches("button")) {
    element = element.parentElement;
  }
  return element;
};

let copyClipboard = (text, event) => {
  const target = findButtonElement(event.target);
  const clipboard = new Clipboard(target, {
    text: () => text,
  });
  clipboard.on("success", () => {
    console.log("copy success", text);
    successMessage(t("detail.copyLinkSuccess"));
  });
  clipboard.onClick(event);
  clipboard.destroy();
};

export default {
  name: "DetailPage",
  components: {
    QrcodeVue,
    Link,
    Sort,
    Message,
    Delete,
    DocumentCopy,
    FolderOpened,
  },
  data: () => {
    return {
      form: {
        text: "",
      },
      dialogFormVisible: false,
    };
  },
  computed: {
    transfers() { return store.transfers; },
    settingForm() {
      return store.settingForm;
    },
    files() {
      return store.files;
    },
    ipFamily() {
      return store.ipFamily;
    },
    netInterfaceNames() {
      return store.netInterfaceNames;
    },
  },
  methods: {
    async formSubmit() {
      await api.addText(this.form.text, this.settingForm.ip);
      store.files = await api.listFiles();
      this.form.text = '';
      this.dialogFormVisible = false;
    },
    async stopServer() {
      await api.stopServer();
      store.serverStatus = 'stop';
      this.$router.replace('/');
    },
    async pickPaths(directory = false) {
      const host = window.otools || window.utools;
      const dialog = host?.dialog || host?.runtime?.dialog;
      if (!dialog?.open) throw new Error('OTools 文件选择不可用');
      const chosen = await dialog.open({ directory, multiple: !directory, title: directory ? t('detail.addFolder') : t('detail.addFiles') });
      const paths = Array.isArray(chosen) ? chosen : chosen ? [chosen] : [];
      for (const path of paths) await api.addFile({ name: pathName(path), path, username: this.settingForm.ip });
      store.files = await api.listFiles();
    },
    async removeFileAll() {
      for (const file of this.files) await api.removeFile(file);
      store.files = await api.listFiles();
      successMessage(t('detail.listCleared'));
    },
    async removeFile(file) {
      await api.removeFile(file);
      store.files = await api.listFiles();
    },
    async openFile(filename) {
      try { await api.openFile(filename); }
      catch (error) { ElMessage.error(t('detail.openFileFail', { err: error.message })); }
    },
    handleFileUrlCopy(file, event) {
      copyClipboard(this.getFileUrl(file), event);
    },
    getFileUrl(file) {
      return `${this.settingForm.url}/api/download?filename=${encodeURIComponent(file.name)}&token=${encodeURIComponent(store.token)}&timestamp=${Date.now()}`;
    },
    handleClipboard(data, event) { copyClipboard(data, event); },
    async changeIpFamily() {
      store.ipFamily = store.ipFamily === 'ipv4' ? 'ipv6' : 'ipv4';
      await api.setIpFamily(store.ipFamily);
      await api.stopServer();
      await api.startServer();
      store.netInterfaceNames = await api.getNetInterfaceNames(store.ipFamily);
      store.currentNetInterfaceIdx = 0;
      store.currentInterfaceName = store.netInterfaceNames[0] || '';
      if (store.currentInterfaceName) await api.setNetInterface(store.currentInterfaceName);
      store.settingForm = await api.getSetting();
      successMessage(t('detail.switchedProtocol', { protocol: store.ipFamily }));
    },
    async changeNetInterface() {
      if (!store.netInterfaceNames.length) return;
      store.currentNetInterfaceIdx = (store.currentNetInterfaceIdx + 1) % store.netInterfaceNames.length;
      store.currentInterfaceName = store.netInterfaceNames[store.currentNetInterfaceIdx];
      await api.setNetInterface(store.currentInterfaceName);
      store.settingForm = await api.getSetting();
      successMessage(t('detail.switchedNetInterface', { name: store.currentInterfaceName }));
    },
  },
};
</script>

<style>
.native-drop-zone { flex-direction: column; border: 1px dashed #a1a9b5; border-radius: 8px; padding: 12px; margin-bottom: 18px; }
.transfer-list { font-size: 12px; color: #637185; margin-bottom: 16px; }
.header-flex {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.share-flex {
  display: flex;
  align-items: center;
  gap: 8px;
}

.share-action {
  flex-shrink: 0;
}

.share-url {
  flex: 1;
  min-width: 0;
  overflow: hidden;
}

.list-header-flex {
  display: flex;
  align-items: center;
  gap: 8px;
}

.list-title {
  flex: 1;
}

.list-action {
  flex-shrink: 0;
}

.url-text {
  display: block;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.detail-space {
  width: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 16px;
}

.upload-box {
  display: flex;
  justify-content: center;
  align-items: center;
}

.el-upload-dragger .el-icon-upload {
  font-size: 46px !important;
  margin: 0 !important;
}

.el-upload-dragger {
  height: 90px !important;
  width: 500px !important;
  margin-bottom: 16px;
}

.file-item {
  margin-bottom: 10px;
}

.username {
  font-size: 12px;
  margin-left: 2px;
  color: #909399;
}

.box-card {
  width: 90%;
}

.el-popover.el-popper {
  min-width: 100px !important;
}
</style>
