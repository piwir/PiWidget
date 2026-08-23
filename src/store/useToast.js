import { reactive } from 'vue';

const toast = reactive({ message: '', error: false, visible: false });
let timer = null;

export function useToast() {
  function show(msg, isError = false) {
    toast.message = String(msg);
    toast.error = isError;
    toast.visible = true;
    clearTimeout(timer);
    timer = setTimeout(() => (toast.visible = false), 2500);
  }
  return { toast, show };
}
