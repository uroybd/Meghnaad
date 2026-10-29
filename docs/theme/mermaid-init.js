// Draws the ```mermaid blocks (the architecture diagram) in the page's theme; mdBook leaves them as code.
(async () => {
  const blocks = document.querySelectorAll('pre > code.language-mermaid');
  if (!blocks.length) return;
  const { default: mermaid } = await import('https://cdn.jsdelivr.net/npm/mermaid@11.4.1/dist/mermaid.esm.min.mjs');
  const dark = !document.documentElement.classList.contains('latte');
  mermaid.initialize({ startOnLoad: false, theme: dark ? 'dark' : 'default' });
  blocks.forEach((code) => {
    const div = document.createElement('div');
    div.className = 'mermaid';
    div.textContent = code.textContent;
    code.parentElement.replaceWith(div);
  });
  await mermaid.run({ querySelector: '.mermaid' });
})();
