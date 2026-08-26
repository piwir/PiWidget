import { ref } from 'vue';

export const now = ref(new Date());
const timer = setInterval(() => (now.value = new Date()), 30000);
if (import.meta.hot) import.meta.hot.dispose(() => clearInterval(timer));
