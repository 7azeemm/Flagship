try {
  const saved = localStorage.getItem('mythos-theme') || localStorage.getItem('comparator_theme');
  document.documentElement.dataset.theme = saved === 'light' ? 'light' : 'dark';
} catch {
  document.documentElement.dataset.theme = 'dark';
}
