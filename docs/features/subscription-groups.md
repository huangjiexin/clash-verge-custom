# Independent subscription groups

The Subscriptions screen has two tabs. The original subscription screen continues
to own URLs, downloads and updates. The new Subscription Groups tab owns named
combinations of existing subscription references and an independent script per
group. Groups run simultaneously and can reuse the same source subscriptions.
There is no fixed limit on the number of sources in a group. Real resource limits
and the original script sandbox's size/time limits still apply.

A source is referenced by its stable UID, selected by display name in the UI.
Subscription URLs and credentials are not copied into the group settings.
The settings and scripts are stored in `subscription-groups.yaml` in the app data
directory. The UI separates source selection, a scrollable code editor and paginated results
into three detail tabs. The UI displays this path, the entire script, its console output,
checks, progress, per-node latency, exclusion reason and last completion time.

Save and Apply validates scripts, atomically saves settings, then uses the normal
upstream configuration activation command. If activation fails, the saved file
remains available to repair; the UI reports that saving and activation had
different outcomes. Workers consume only the committed runtime's requests.

## Script contract

The existing upstream sandbox invokes `main(config, groupName)`. `config.proxies`
contains the group's source nodes, with stable names scoped by group and source.
A script can filter nodes or reorder them. It must preserve unique imported names.
The returned `config.screening` declares optional network checks:

```javascript
function main(config) {
  config.proxies = config.proxies.filter(p => /US|美国/.test(p.name));
  config.screening = {
    interval: 300,
    concurrency: 16,
    "interface-name": "",
    sort: "delay",
    automatic: true,
    checks: [
      {url: "https://example.com/health", "expected-status": 200, validation: "status"}
    ]
  };
  return config;
}
```

`checks: []` means script-only selection; it makes no connectivity claim.
`sort` is `delay`, `name`, or `original`. Automatic selection requires a check.
The script sandbox does not perform arbitrary networking: the independent worker
executes the declared GET checks through each node, with TLS verification,
no redirects, a 10-second per-request timeout and a 1 MiB response limit.
All checks must pass. `status` checks the configured HTTP status; `auth-json`
requires an authentication-error JSON response with 401; `html` requires 200 HTML
without the recognized challenge markers. Templates are editable examples,
not embedded business policies. Built-in starters cover merging, protocol filtering,
a single HTTP target and multiple targets. Replace reserved `.example` URLs with
your own endpoints and expected responses before applying network templates.
Network-check templates require the user to fill in the physical outbound
interface for their device; selection-only templates can leave it empty.
No platform-specific adapter name is assumed.

The help dialog separates getting started, script reference and local storage.
The configuration path is resolved from the current user's OS data directory at
runtime; no personal absolute path or subscription URL is shipped in the feature.
Moving group settings between devices requires importing and reselecting sources.

The isolated bundled Mihomo has TUN disabled and loopback-only proxy listeners.
By default 16 nodes are tested concurrently. Each group can set `concurrency`
from 1 to 32; checks within each node remain sequential and fail fast. Groups
share the scan worker and queue, so their limits do not multiply.
The process and private configuration are removed after the scan or cancellation.
No user-managed service, system proxy change, or rule change is needed.

Each group has independent fingerprints, timestamps and failure state. A successful
full check admits a node, a failed full check removes it, and only a later full
success readmits it. The first check supplies the health-check URL and initial
latency. Native health checks run every 60 seconds; the worker observes failures
within 2–5 seconds, even during scans. UI Error/Timeout measurements are hidden
when the measurement batch settles. Unknown measurements are not failures.
Automatic selection is pinned first, and `REJECT` remains visible if none qualify.
The full scan interval (60–3600 seconds) starts after the preceding scan finishes.
Source files and scripts are re-evaluated at the next scan and during normal config
regeneration. Changed credentials or check conditions invalidate old passes.

Scripts cannot change the active profile's routing rules through this interface.
Users route traffic to the named groups using their existing rule editor.
Broken scripts or missing sources produce an empty rejecting group and a visible
error rather than borrowing another group's nodes. This version supports inline
`proxies`; sources using `proxy-providers` report an explicit unsupported-source
error. Traffic/expiry placeholders and dependent `dialer-proxy` nodes are excluded.

## Upstream rebase boundary

All feature implementation is under:

- `src-tauri/src/extensions/subscription_groups/`: settings, adapter, worker and IPC.
- `src/features/subscription-groups/`: tabs, editor, templates, command service and
  group-specific presentation policy.

The upstream integration points are deliberately additive:

- `lib.rs` and `ipc.rs`: module and command registration.
- `config/config.rs`, `runtime.rs`, manager fallback: post-enhancement composition
  and a committed request list, without replacing upstream transactions.
- `enhance/mod.rs` and `script.rs`: expose the existing sandbox; do not fork it.
- Timer startup and exit cleanup: worker lifetime hooks.
- Proxy-view command/model: optional extension policy and URL-specific history.
- Navigation and subscription page title: tab wrapper around the unchanged page.
- Proxy list rendering and delay service: optional policy calls; ordinary groups
  retain upstream behavior. Generated IPC bindings are regenerated, never edited.

On rebase, review these small hooks and rerun the extension tests, IPC generator,
frontend typecheck/build and native build. This separation reduces conflicts; it
does not guarantee conflict-free upgrades when upstream interfaces change.

Existing `codex-screening` profile-script declarations are a compatibility adapter.
If no independent settings file exists, they appear as an editable legacy group.
Saving the new tab writes independent settings; after that the tab is authoritative.
No subscription source IDs or URLs are built into the engine.

Proxy cards and summaries use separate display labels supplied by the extension.
Core names remain stable for selection, checks and routing; labels preserve the
original node and source names, including slashes. Automatic entries display as
“自动选择” within their parent and “<group> · 自动选择” elsewhere.
