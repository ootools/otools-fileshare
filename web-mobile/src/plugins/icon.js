import SvgIcon from '@/components/SvgIcon/index.vue';
import FileIcon from '@/components/FileIcon/index.vue';

export function registryIcon(app) {
  app.component('svg-icon', SvgIcon);
  app.component('file-icon', FileIcon);
}
