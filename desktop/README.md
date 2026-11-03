# Cadence desktop

A Mac focus app organized around Today, All tasks, Capacity, and a Focus Journal, with a natural language task assistant. Based on [NeonSupernova/tauri-app](https://github.com/NeonSupernova/tauri-app); see [UPSTREAM.md](UPSTREAM.md) for the exact template snapshot and retrieval details.

The app follows system light/dark appearance. On macOS it uses native NSVisualEffectView sidebar material, not a CSS imitation of Liquid Glass. It reads Apple's Reduce Transparency, Reduce Motion, and Increase Contrast settings on launch/focus and through AppKit display-change notifications; transparency/contrast preferences switch the app to opaque surfaces. Its normal activation policy keeps it in the Dock. Closing the window hides it; click the Dock icon to reopen, or use Quit to exit. The bundle includes a custom app icon.

Start a task with its play button or say “Start TASK-001.” Pause with the timer controls or “Pause TASK-001.” Starting another task automatically pauses the previous timer in the same workspace. Completing, reopening, or trashing a running task saves elapsed time. Task details show total tracked time and session history; the summary shows today/all-time totals. Sort by Time tracked, or ask chat about time spent. Timers continue while windows are closed and across app restarts. AppKit’s will-sleep notification pauses running timers before Mac sleep; waking does not automatically resume a task. Existing tasks load with zero tracked time and no running timer.

Canvas integration:

1. Open **Settings → Canvas** and enter your Canvas site URL (defaults to `https://pcci.instructure.com`).
2. Generate a personal access token in **Canvas → Account → Settings → New Access Token**, then paste it into the password field.
3. Choose **Save & sync assignments**. An OpenRouter key is not needed for Canvas sync.

The app reads all published assignments in your active student courses, follows every pagination page, applies personalized due-date overrides, and requests only your own submission metadata. It creates assignment tasks with `canvas`, `school`, and `course:<course-code>` labels. The `canvas` label stays attached to imported assignments even after local label edits. Use the Canvas sidebar entry or “View assignments” in the connection report to open the real assignment workspace from demo mode.

Sync runs when the app launches and every 15 minutes while enabled; use the sidebar refresh button or type **“sync Canvas”** in chat to sync immediately. It never submits, grades, comments on, or edits anything in Canvas. Local completion is not a Canvas submission. Submitted, graded, or excused assignments are imported as complete; a newly observed Canvas submission completes a previously unsubmitted task and stops its timer. Explicit local completion/reopening is preserved on subsequent unchanged syncs. Old Canvas submission dates are used for progress statistics rather than the import time.

A Canvas site/course/assignment key prevents duplicates and keeps task IDs stable. Sync updates fields that still match their previous imported values and preserves customized titles, descriptions, dates, priorities, timers, and Trash. Failed fetches or invalid imports do not partially commit a batch. Deleted or unavailable remote assignments are not automatically removed from your local tracker.

Assignment details show the original Canvas deadline, course, submission state, and an **Open assignment in Canvas** action using the Mac's default browser. Assignment descriptions are imported as readable plain text, including paragraphs and list content, and appear in the task card. Resync updates descriptions that have not been customized locally, including older placeholder descriptions. Rubrics and attachments remain in Canvas. Imported descriptions become part of task context when you use OpenRouter chat.

Credentials are stored in `canvas-settings.json` under app data with Unix permissions 0600. Settings responses return only connection status, never the token. Runtime `.env` / environment variables `CANVAS_BASE_URL` and `CANVAS_API_TOKEN` are supported. HTTPS requests run in Rust, pass the bearer token through stdin, do not follow redirects, and only follow pagination links on the same origin and requested API path. No Canvas credentials are sent to OpenRouter. One Canvas connection is supported per local workspace.

Live Canvas synchronization still requires your token and network access; automated verification uses synthetic API fixtures. References: [Canvas assignments API](https://developerdocs.instructure.com/services/canvas/resources/assignments), [pagination](https://developerdocs.instructure.com/services/canvas/basics/file.pagination), [token setup](https://developerdocs.instructure.com/services/canvas/oauth2/file.oauth).

Mac integrations are implemented in `src-tauri/src/macos.rs`:

- A native menu bar status item shows the real workspace’s running timer, with Pause, Complete, Quick Capture, and Open controls. It keeps working with every webview hidden. Demo timers stay isolated from this item.
- A separate floating Quick Capture window adds exactly one new task. Rust rejects mutation plans from that entry point. It uses the real workspace and the same Jev/OpenRouter pipeline.
- Standard macOS application, File, Edit, Task and Window menus preserve system text editing, Services, Hide and Quit behavior.
- Native context menus let you focus, pause, complete, edit, trash, restore, or copy a task reference. Clipboard support only writes through NSPasteboard; it never reads your clipboard.
- NSWorkspace notifications update accessibility display settings and pause timers on sleep. Notification observers unregister when their owning main-thread objects are released.
- The main window keeps native traffic lights in an overlay title bar with draggable toolbar regions.

The tracker opens in **Today**, showing remaining focused time and work units, a next suggestion, and suggested work blocks. **All tasks** keeps the sortable task table. **Capacity** shows a seven-day plan, editable weekday budgets, energy check-ins, and learning feedback. **Focus Journal** shows timestamped session history. Chat and task details open on demand.

Starting capacity is **2 hours on weekdays and 1 hour on weekends**. Task assessments score estimated time, technical difficulty, tedium, ambiguity, and switching cost. Tracked completions and explicit effort/day feedback adapt estimates and weekday capacity. Deadline pressure affects order separately from effort cost. See [WORKLOAD.md](WORKLOAD.md) for the algorithm, observation rules, and prototype limitations.

| Shortcut | Action |
| --- | --- |
| ⌘N | New task in the conversation |
| ⇧⌘N | Quick Capture window |
| ⌘F | Find a task |
| ⌘↵ | Start/pause the selected task, or the Today task |
| ⇧⌘↵ | Complete the selected/running task |
| ⇧⌘P | Pause the real workspace’s running timer |
| ⌘1 / ⌘2 / ⌘3 | Today / All tasks / Focus Journal |
| ⌘, | Settings |
| ⌘K | Focus the conversation |

These are application menu shortcuts while Cadence is active, not global hotkeys. Quick Capture is also available from the menu bar while the main window is hidden.

On macOS, double-click `launch.command` to install dependencies, build the `.app`, and open it. Requires Bun, Rust, Xcode command-line tools, and internet access for the first build.

Or run:

```sh
cd desktop
bun install
bun run launch
```

For native development, use `bun run tdev`. To create only the application bundle, use `bun run tbuild -- --bundles app`. The macOS bundle is `src-tauri/target/release/bundle/macos/Cadence.app`.

The root `.env` is read by Rust at runtime during development. Existing `OPENROUTER_API_KEY`, `OPENROUTER_MODEL`, `JEV_MODEL`, `REASONING_EFFORT`, and `LABEL_THRESHOLD` settings are supported. You can also enter your key and model preferences in the app's Settings. Settings store the key locally with file mode 0600 on Unix. The key is never returned to the frontend or embedded at build time. Chat sends task context and recent conversation to OpenRouter. HTTPS uses the operating system's `curl` executable from Rust, with credentials and JSON passed through stdin.

Try chatting naturally:

- “Finish my project proposal by Friday.”
- “Show my high priority coding tasks, sorted by due date.”
- “Mark TASK-001 done.”
- “Move the dentist appointment to next Tuesday.”
- “Add the label launch to TASK-002 and make it high priority.”
- “Rename TASK-003 to Plan our Chicago weekend.”
- “Start the checkout task.”
- “Delete TASK-004.” / “Restore TASK-004.”
- “What did I get done this week?”

Click a task ID or its reference arrow to mention it in chat. The currently selected task and recent conversation also help resolve “this task” and follow-up requests. Ambiguous references are instructed to produce clarification rather than mutations; this semantic behavior still needs live-model verification. Explicit IDs are validated by Rust.

Jev uses typed Choice, Noul and Score questions for intent, type, multiple labels, priority, estimated time, and difficulty factors. OpenRouter's reasoning model produces a validated action plan and useful task titles/descriptions. Rust executes plans in a SQLite transaction: invalid IDs, priorities, types, or dates roll back the entire batch. Deletion moves tasks to Trash and keeps references stable. Completion, reopening, starting tasks, editing, labeling, filtering, sorting, and statistics are available through chat. Common actions also have visible controls.

Rust owns persistence, statistics, task filtering/sorting, plan validation, and mutations. Svelte renders the returned state. Tasks and conversation history live in the platform's app data directory (`~/Library/Application Support/com.infernalnova.jev-tasks` on macOS). The demo uses a separate SQLite database. The earlier console prototype's database is left intact; it is not automatically imported.

`Try demo` opens a separate sample workspace without touching real tasks; chat in it still needs a real OpenRouter key. Running the frontend in an ordinary browser displays clearly labeled, display-only fixtures for visual review. Mutations and chat require the native Tauri app; browser preview does not simulate successful operations.

Verification:

```sh
bun run check
bun run build
bun run test:rust
cargo clippy --manifest-path core/Cargo.toml --all-targets -- -D warnings
```

The Rust core has forty-four integration tests, covering adaptive workload and capacity behavior, timer persistence, exact elapsed time, switching timers, rollback, and older-data compatibility. Svelte checks and the production frontend build pass. Native compilation and strict Clippy for the Tauri app pass using cached dependencies. Desktop runtime appearance still needs visual verification on macOS; the sandbox does not allow launching GUI processes.

Apple references: [Mac sleep notifications](https://developer.apple.com/documentation/appkit/nsworkspace/willsleepnotification), [Reduce Transparency](https://developer.apple.com/documentation/appkit/nsworkspace/accessibilitydisplayshouldreducetransparency). Native materials follow the system appearance; this app currently uses the Tauri-supported sidebar material rather than the newer NSGlassEffectView API.


## Simple connected work

Click a task title or conversation reference to open its card. Overview contains its description, labels, actions, and connected work. Links & planning holds optional organization. Effort & time holds estimates, tracking, and learning feedback.

Create with chat starts guided capture: the assistant asks about the outcome and missing scope, suggests labels, and prepares a description. Explicit walkthrough requests ask for confirmation before creation. Clear direct task requests can still create immediately. This behavior is instructed in the model prompt; live-model dialogue needs verification with an OpenRouter key. Rust rejects empty new descriptions and supplies an inbox label if no relevant category was supplied.

Every item keeps a stable TASK reference. Its optional role is task, epic, milestone, sprint, or iteration. Tasks can belong to an epic, target a milestone, and sit in a dated sprint/iteration. Related-item links are visible in both directions. Nested epics are supported, with cycle prevention. Planning items do not consume focus capacity or support task timers. Completing an epic does not automatically complete its children. Planning windows describe organization; task deadlines and the seven-day capacity plan remain separate.

External references carry a provider name, remote ID, and URL. This supports links to GitHub, GitLab, OpenProject, Jira, and other trackers without forcing their terminology into local tasks. These references open in the default macOS browser; remote connectors, authentication, two-way sync, and remote writes are not implemented. Canvas remains the implemented read-only importer.

Links and planning fields can also be edited through chat. The assistant uses existing IDs only; creating a container and linking newly created children takes separate turns because IDs are assigned after creation. Invalid links, circular epic hierarchies, incompatible role changes, and invalid planning dates roll back the full batch.


## Responsiveness and app identity

Cadence uses graphite surfaces and blue accents in both system appearances, with a matching app icon. The macOS bundle identifier and existing app-data directory remain unchanged so renaming does not move tasks, timers, Canvas credentials, or assistant settings.

Chat and effort assessments perform network requests outside the task-operation mutex. Task controls remain usable while the assistant thinks; the conversation shows elapsed waiting time. A delayed AI mutation is rejected if task state changed in the meantime, with a retry message instead of overwriting newer edits.

Database initialization runs once per schema version instead of reissuing schema writes on every snapshot. SQLite uses WAL, a bounded busy timeout, and immediate write transactions. Regression tests cover reading the initialized database while a separate connection holds an uncommitted write, and rejecting stale AI edits. GUI responsiveness and live-provider delays still require runtime validation.
