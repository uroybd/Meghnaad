// Draws the ```mermaid blocks (the architecture diagram) in the page's theme; mdBook leaves them as code.
(async () => {
  const blocks = document.querySelectorAll('pre > code.language-mermaid');
  if (!blocks.length) return;
  const [{ default: mermaid }, { default: elk }] = await Promise.all([
    import('https://cdn.jsdelivr.net/npm/mermaid@11.17.2/dist/mermaid.esm.min.mjs'),
    import('https://cdn.jsdelivr.net/npm/@mermaid-js/layout-elk@0.2.3/dist/mermaid-layout-elk.esm.min.mjs'),
  ]);
  // ELK lays out the whole graph at once: subgraphs side by side and square edges, where dagre scatters them.
  mermaid.registerLayoutLoaders(elk);
  const dark = !document.documentElement.classList.contains('latte');
  mermaid.initialize({ startOnLoad: false, theme: dark ? 'dark' : 'default', layout: 'elk' });
  blocks.forEach((code) => {
    const div = document.createElement('div');
    div.className = 'mermaid';
    div.textContent = code.textContent;
    code.parentElement.replaceWith(div);
  });
  await mermaid.run({ querySelector: '.mermaid' });
})();
