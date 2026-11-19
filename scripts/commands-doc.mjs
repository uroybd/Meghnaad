// The commands of the `:` line (spec 0020) are described once, in `web/public/keymaps/commands.json`: what each front
// end lists, and this page of the documentation, `docs/commands.md`, which is made from it with an anchor for each
// command (`commands.html#tabnext`) that the front ends link to. `node scripts/commands-doc.mjs` writes the page; a test
// fails when it is out of step with the file. `stampSince` writes a release's version over `unreleased`, at the bump.
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { ROOT } from './deploy-lib.mjs';

/** Where the file is, and the copy the phone carries (made by `scripts/gen-lucide.mjs`). */
export const COMMANDS_FILES = [
  'web/public/keymaps/commands.json',
  'app/shared/src/commonMain/resources/keymaps/commands.json',
];
export const COMMANDS_DOC = 'docs/commands.md';

/** A command as vim writes it, the part that may be left out in brackets: `tabn[ext]`. */
export function written(c) {
  return c.shortest < c.name.length ? `${c.name.slice(0, c.shortest)}[${c.name.slice(c.shortest)}]` : c.name;
}

const since = (v) => (v === 'unreleased' ? 'the next release' : v);

/** The page for `data`, the parsed `commands.json`. */
export function commandsDoc(data) {
  const out = [
    '# Commands',
    '',
    '<!-- Made by scripts/commands-doc.mjs from web/public/keymaps/commands.json: change that file, not this page. -->',
    '',
    'A line of the command line that starts with `:` joined to one of these runs it, in the terminal app and the web',
    "app; any other line goes to `task`. They are vim's where vim has them, and may be written as short as vim allows:",
    'the part in brackets may be left out (`:tabn` for `:tabnext`). A key can run one too, with its words, in place of an',
    'action: `"tasks": { "g h": "filter +home" }` in [settings.json](using/settings.md#keys). The task commands act on',
    'the marked tasks when there are any, and else on the chosen one.',
    '',
    '| Command | Does |',
    '| --- | --- |',
    ...data.commands.map((c) => `| [\`:${written(c)}\`](#${c.name}) | ${c.summary} |`),
    ...data.lines.map((l) => `| ${l.written.map((w) => `\`${w}\``).join(', ')} | ${l.summary} |`),
    '',
  ];
  for (const c of data.commands) {
    out.push(`<a id="${c.name}"></a>`, '', `## :${written(c)}`, '');
    if (c.takes) out.push(`Takes ${c.takes}.`, '');
    out.push(c.doc, '', `Since ${since(c.since)}.`, '');
  }
  out.push('## Rows', '');
  for (const l of data.lines) out.push(`${l.written.map((w) => `\`${w}\``).join(', ')}: ${l.doc}`, '');
  return out.join('\n');
}

/** `data` with every `since` that is `unreleased` set to `version`. */
export function stampSince(data, version) {
  const stamp = (e) => (e.since === 'unreleased' ? { ...e, since: version } : e);
  return { ...data, commands: data.commands.map(stamp), lines: data.lines.map(stamp) };
}

const read = (root, file) => JSON.parse(readFileSync(join(root, file), 'utf8'));

/** Write the page from the file. */
export function writeCommandsDoc(root = ROOT) {
  writeFileSync(join(root, COMMANDS_DOC), commandsDoc(read(root, COMMANDS_FILES[0])));
}

/** Give the commands brought in since the last release `version`, in every copy, and write the page again. */
export function stampCommands(version, root = ROOT) {
  for (const file of COMMANDS_FILES)
    writeFileSync(join(root, file), JSON.stringify(stampSince(read(root, file), version), null, 2) + '\n');
  writeCommandsDoc(root);
}

/** The commands and lines of every copy still `unreleased`, as `file: name` (none once a release has stamped them). */
export function unreleased(root = ROOT) {
  return COMMANDS_FILES.flatMap((file) => {
    const data = read(root, file);
    return [...data.commands.map((c) => [c.name, c.since]), ...data.lines.map((l) => [l.written.join(' '), l.since])]
      .filter(([, since]) => since === 'unreleased')
      .map(([name]) => `${file}: ${name}`);
  });
}

if (import.meta.url === `file://${process.argv[1]}`) {
  // `released` fails while a command says `unreleased` (the Release workflow, for a tag); else write the page.
  if (process.argv[2] === 'released') {
    const left = unreleased();
    if (left.length) {
      console.error(`Still "unreleased" (node scripts/version.mjs sync writes the version):\n${left.join('\n')}`);
      process.exit(1);
    }
  } else writeCommandsDoc();
}
