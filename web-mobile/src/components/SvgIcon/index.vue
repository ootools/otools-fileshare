<template>
  <div class="svg-icon-wrapper" :style="iconStyle">
    <img class="svg-icon" :style="iconStyle" :src="iconUrl" alt="" aria-hidden="true" />
  </div>
</template>

<script>
const icons = import.meta.glob('../../assets/icons/svg/*.svg', { eager: true, query: '?url', import: 'default' });
export default {
  computed: {
    iconUrl() { return icons[`../../assets/icons/svg/${this.name}.svg`] || icons['../../assets/icons/svg/file-other.svg']; },
  },
  name: "SvgIcon",
  props: {
    name: {
      // svg文件名称
      type: String,
      required: true,
    },
    iconStyle: {
      type: String,
      required: false,
      default: 'height: 16px; weight: 16px;'
    }
  },
};
</script>

<style scoped>
.svg-icon-wrapper {
  display: inline-block;
}
.svg-icon {
  width: 2em;
  height: 2em;
  vertical-align: -0.15em;
  fill: currentColor;
  overflow: hidden;
  pointer-events: none;
}
</style>
