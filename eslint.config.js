// Lints the web app (TypeScript + Svelte 5) and the Node scripts. Formatting is Prettier's job (`npm run format`),
// so no style rules here.
import js from '@eslint/js';
import globals from 'globals';
import svelte from 'eslint-plugin-svelte';
import svelteParser from 'svelte-eslint-parser';
import ts from 'typescript-eslint';

export default ts.config(
  {
    ignores: [
      'node_modules/**',
      'target/**',
      'crates/**',
      'docs/**',
      'web/dist/**',
      'web/.svelte-kit/**',
      '.wrangler/**',
    ],
  },
  js.configs.recommended,
  ...ts.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    // The Svelte 5 app runs in a browser.
    files: ['web/**/*.{ts,js,svelte}'],
    languageOptions: { globals: globals.browser },
  },
  {
    // The deploy and build scripts run in Node.
    files: ['scripts/**/*.mjs', '*.config.js'],
    languageOptions: { globals: globals.node },
  },
  {
    // `.svelte` and `.svelte.ts` files: Svelte's parser, with TypeScript inside.
    files: ['**/*.svelte', '**/*.svelte.ts'],
    languageOptions: { parser: svelteParser, parserOptions: { parser: ts.parser } },
  },
  {
    rules: {
      // `_name` marks an argument that is there only for its position.
      '@typescript-eslint/no-unused-vars': [
        'error',
        // `({ aud, ...rest }) => rest` drops a key on purpose.
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_', ignoreRestSiblings: true },
      ],
      // The Sets, Maps and Dates here are scratch values, or replacements assigned whole to `$state`
      // (`const next = new Set(open); …; open = next`): never mutated in place, so a reactive one buys nothing.
      'svelte/prefer-svelte-reactivity': 'off',
      // `svelte-ignore state_referenced_locally` marks places that read a prop once, on purpose, as the start of a
      // form. That warning comes from the Svelte compiler with TypeScript, which this rule can't see, so it reports
      // them as unused. `svelte-check` is what proves they are needed (it warns the moment one is removed).
      'svelte/no-unused-svelte-ignore': 'off',
    },
  },
);
