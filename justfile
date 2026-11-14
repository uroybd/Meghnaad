# Common tasks for the whole repository. `just` lists them. They wrap the npm scripts and the scripts in `scripts/`, which
# stay the source of truth (CI and the Release workflow call those, not this file).

set shell := ["bash", "-euo", "pipefail", "-c"]

# List the recipes
default:
    @just --list --unsorted

# --- Working on it ---

# The dev server: the engine (rebuilt when its Rust changes), wrangler dev --local and Vite
dev *args:
    npm run dev -- {{ args }}

# Build the engine and the web app
build:
    npm run build

# Build the Android APK (pass --release to sign it)
android *args:
    node scripts/build-android.mjs {{ args }}

# Run the terminal app on your own task data
tui *args:
    cargo run --release -p meghnaad-tui -- {{ args }}

# Build the documentation site into docs/book
docs:
    mdbook build docs

# Free disk: old builds of our crates (`debug`: the whole debug build; `all`: every Rust, Android and docs build)
cleanup what="ours":
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ what }}" in
      ours) node scripts/prune-target.mjs ;;
      debug) cargo clean --profile dev ;;
      all)
        cargo clean
        rm -rf app/build app/shared/build app/androidApp/build app/desktopApp/build docs/book
        ;;
      *) echo "The argument is \`ours\`, \`debug\` or \`all\`, not \`{{ what }}\`." >&2; exit 1 ;;
    esac
    df -h . | tail -1

# --- Checks (what CI runs, in AGENTS.md's order) ---

# Fix what clippy, rustfmt and Prettier can fix themselves
format:
    npm run format

# rustfmt and clippy (engine, Worker, browser engine, phone, hook, TUI), Prettier and ESLint
lint:
    npm run lint

# The engine's and the terminal app's tests, the web tests, the build and deploy scripts' tests
test:
    cargo test
    npm --prefix web test
    node --test scripts/*.test.mjs

# svelte-check, and the Worker and browser engine built for wasm32
typecheck:
    npm run check
    cargo check -p tw-worker --target wasm32-unknown-unknown
    cargo check -p tc-browser --target wasm32-unknown-unknown

# The Kotlin app's tests and a debug APK (builds the engine for this machine first)
test-android:
    CARGO_PROFILE_ANDROID_STRIP=none cargo build --profile android -p tc-android
    cd app && ./gradlew :shared:testDebugUnitTest :androidApp:assembleDebug

# Lint and test, fixing nothing: what a release can run first (`just ship patch tidy`)
tidy: lint test

# Everything before a change is done: format, lint, test, typecheck
check: format lint test typecheck

# --- Versions and releases ---

# The version every part carries (the root package.json's)
version:
    @node -p "require('./package.json').version"

# Set the version everywhere: patch, minor, major or an exact 1.2.3 (Cargo.toml and Cargo.lock follow)
bump level="patch":
    npm version {{ level }} --no-git-tag-version --silent >/dev/null
    @echo "Version $(just version)"

# The release notes for what is on main now: commits since the last tag, by conventional type
notes ref="HEAD":
    @node scripts/release-notes.mjs {{ ref }}

# Bump, commit, tag v<version> and push main and the tag; `just release minor tidy` lints and tests first
release level="patch" tidy="": _clean
    #!/usr/bin/env bash
    set -euo pipefail
    case "{{ tidy }}" in
      "") ;;
      tidy) just tidy ;;
      *) echo "The second argument is \`tidy\` or nothing, not \`{{ tidy }}\`." >&2; exit 1 ;;
    esac
    just bump {{ level }}
    # The bump makes every build of our own crates stale (the version is in their hash); nothing would use them again.
    node scripts/prune-target.mjs
    git add package.json package-lock.json Cargo.toml Cargo.lock
    git commit -q -m "chore: bump to $(just version)"
    git tag "v$(just version)"
    git push -q origin main "v$(just version)"
    echo "Released v$(just version)"

# Deploy main with the Deploy workflow (it does not wait for CI)
deploy:
    gh workflow run deploy.yml --ref main
    @echo "Deploy started: gh run list --workflow deploy.yml"

# Release, then deploy. `just ship patch tidy` lints and tests first
ship level="patch" tidy="": (release level tidy) deploy

# On main, nothing uncommitted, and nothing on origin/main that isn't here
[private]
_clean:
    #!/usr/bin/env bash
    set -euo pipefail
    branch=$(git branch --show-current)
    [ "$branch" = main ] || { echo "Release from main, not $branch." >&2; exit 1; }
    git diff --quiet && git diff --cached --quiet || { echo "Commit or stash your changes first." >&2; exit 1; }
    git fetch -q origin main
    behind=$(git rev-list --count HEAD..origin/main)
    [ "$behind" = 0 ] || { echo "origin/main has $behind commit(s) this branch lacks: pull first." >&2; exit 1; }
