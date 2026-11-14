---
name: sync-guidelines
description: Refresh a pkgforge-apps app's copy of the shared guidelines, skills, scripts and workflows from pkgforge-apps/.github, or change the shared copy itself. Use when the shared files changed, when a sync pull request needs attention, when a rule, skill, script or workflow should apply to every app, or when one of them looks out of date in an app.
---

# Shared files

What every app has in common lives once, in
[pkgforge-apps/.github](https://github.com/pkgforge-apps/.github):

```
agents/
|-- GUIDELINES.md        the shared part of every app's AGENTS.md
`-- skills/<name>/       the shared skills
app/                     files copied into every app as they are
|-- scripts/             check, package, icons and this sync
`-- .github/workflows/   callers of the shared reusable workflows
.github/workflows/app-*  the reusable workflows: CI, release-plz,
                         asset builds, docs, this sync
```

Each app carries a copy, so agents and local runs use it directly:

- `AGENTS.md` holds the guidelines between the
  `<!-- BEGIN SHARED GUIDELINES` and `<!-- END SHARED GUIDELINES`
  markers. What follows the end marker is the app's own section.
- `.agents/skills/<name>/` holds the shared skills next to any the app
  has of its own. `.claude/skills/<name>` is a symlink to each, since
  Claude Code reads skills only from there; the sync creates missing
  links, for the app's own skills too.
- Every file under `app/` sits at the same path in the app. The synced
  workflows call `pkgforge-apps/.github/.github/workflows/app-*.yml`
  pinned to the commit they were synced from, so an app's CI and
  releases change only when its sync pull request is merged.
- `.agents/shared-guidelines.lock` records the revision synced and the
  skills and files that came from it.

## Refresh an app

```sh
sh scripts/sync-guidelines.sh
```

It downloads `main` of pkgforge-apps/.github, rewrites the marked block,
replaces the shared skills and files, pins the workflows to that commit,
and removes shared skills and files that no longer exist upstream.
`PKGFORGE_SHARED_REF=<ref>` syncs from another branch, tag or commit.
Review the diff and commit it as `chore: sync shared guidelines`.

It stops before writing anything when it would lose something of the
app's: a shared file, skill or the marked block that differs from what
the last sync wrote, or a path the new revision adds that the app
already has. It lists each one. Move the change upstream or into the
app's own files, or sync with `PKGFORGE_SYNC_OVERWRITE=1` to drop it.

The `sync-apps` workflow of pkgforge-apps/.github runs it in every app
whenever `agents/`, `app/` or a reusable `.github/workflows/app-*.yml`
changes there, and opens or updates a
`chore/sync-guidelines` pull request with the result as
pkgforge-apps-bot, so apps rarely need it by hand. The bot's pull
requests start the app's CI like anyone's. An app whose sync stops shows
up as a failed `sync-apps` run, with the list in its log.

## Change a shared file

1. Edit `agents/` in a checkout of pkgforge-apps/.github. Keep it
   app-agnostic: no app names, and paths only from the shared layout.
2. Try it on an app before it lands:

   ```sh
   PKGFORGE_SHARED=/path/to/.github sh scripts/sync-guidelines.sh
   ```

   The lock then records a local checkout; do not commit that sync.
3. Land the change in pkgforge-apps/.github. `sync-apps` opens a pull
   request in every app right away. To rerun it, for an app added since
   or after fixing a failed run:
   `gh workflow run sync-apps.yml -R pkgforge-apps/.github`.

## Rules

- Never edit the marked block, a shared skill, or a file the lock lists
  inside an app. The next sync stops on it. Make the change upstream.
- Something only one app needs goes in that app's own section of
  `AGENTS.md`, or in a skill or file the shared set does not use.
- A change to a reusable workflow in `.github/workflows/` reaches an app
  only through the pin, so it takes effect when that app's sync pull
  request is merged.
- When a rule turns out to help every app, move it upstream and delete
  the app's copy.
