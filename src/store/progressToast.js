import { reactive } from 'vue';

export const progressToastState = reactive({
  visible: false,
  text: '',
  percent: null,
  error: false,
});

export function showProgress(text, percent = null) {
  progressToastState.visible = true;
  progressToastState.error = false;
  progressToastState.text = text;
  progressToastState.percent = percent;
}

export function showProgressError(text) {
  progressToastState.visible = true;
  progressToastState.error = true;
  progressToastState.text = text;
}

export function hideProgress() {
  progressToastState.visible = false;
}
