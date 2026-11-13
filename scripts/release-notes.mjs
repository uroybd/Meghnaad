// What changed in a release, for its GitHub release notes: the commits since the tag before it, grouped by their
// conventional type (`feat(web): …` is a feature of the web app). Commits go straight to main, so GitHub's own notes
// (pull requests) are empty; a commit's subject is its note. Chores, CI, tests, style and the version bumps are left out.
//   node scripts/release-notes.mjs [tag]   the notes for that tag (default: HEAD, as if it were released now)
import { execFileSync } from 'node:child_process';
import { ROOT } from './deploy-lib.mjs';

/** The groups a release's notes have, in order, and the commit types each takes. */
const GROUPS = [
  ['Features', ['feat']],
  ['Fixes', ['fix']],
  ['Performance', ['perf']],
  ['Under the hood', ['refactor']],
  ['Documentation', ['doc', 'docs']],
];

const SUBJECT = /^(\w+)(?:\(([^)]+)\))?(!)?:\s*(.+)$/;

/** One commit subject as `{ type, scope, breaking, text }`, or null when it is not a conventional one. */
export function parseSubject(subject) {
  const m = SUBJECT.exec(subject.trim());
  if (!m) return null;
  return { type: m[1].toLowerCase(), scope: m[2] ?? null, breaking: !!m[3], text: m[4] };
}

/** The notes for these commits (`{ sha, subject }`, newest first, as `git log` gives them): Markdown, empty for none. */
export function releaseNotes(commits) {
  const lines = [];
  for (const [title, types] of GROUPS) {
    const items = [];
    for (const { sha, subject } of commits) {
      const c = parseSubject(subject);
      if (!c || !types.includes(c.type)) continue;
      const scope = c.scope ? `**${c.scope}:** ` : '';
      const breaking = c.breaking ? '**Breaking:** ' : '';
      items.push(`- ${breaking}${scope}${c.text} (${sha.slice(0, 7)})`);
    }
    // Oldest first reads as the story of the release.
    if (items.length) lines.push(`### ${title}`, '', ...items.reverse(), '');
  }
  return lines.join('\n').trimEnd();
}

const git = (...args) => execFileSync('git', args, { cwd: ROOT, encoding: 'utf8' }).trim();

/** The release tag before `ref` (`v*`), or null for the first release. */
export function previousTag(ref) {
  try {
    return git('describe', '--tags', '--abbrev=0', '--match', 'v*', `${ref}^`);
  } catch {
    return null;
  }
}

/** The commits in `ref` since the release before it, newest first. */
export function commitsFor(ref) {
  const prev = previousTag(ref);
  const out = git('log', '--format=%H%x09%s', prev ? `${prev}..${ref}` : ref);
  return out
    ? out.split('\n').map((l) => {
        const [sha, subject] = l.split('\t');
        return { sha, subject };
      })
    : [];
}

if (import.meta.url === `file://${process.argv[1]}`) {
  process.stdout.write(`${releaseNotes(commitsFor(process.argv[2] ?? 'HEAD'))}\n`);
}
