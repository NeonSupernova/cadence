# Cadence desktop

A Mac focus app organized around Today, All tasks, Capacity, and a Focus Journal, with a natural language task assistant. Based on [NeonSupernova/tauri-app](https://github.com/NeonSupernova/tauri-app); see [UPSTREAM.md](UPSTREAM.md) for the exact template snapshot and retrieval details.

The app follows system light/dark appearance. On macOS it uses native NSVisualEffectView sidebar material, not a CSS imitation of Liquid Glass. It reads Apple's Reduce Transparency, Reduce Motion, and Increase Contrast settings on launch/focus and through AppKit display-change notifications; transparency/contrast preferences switch the app to opaque surfaces. Its normal activation policy keeps it in the Dock. Closing the window hides it; click the Dock icon to reopen, or use Quit to exit. The bundle includes a custom app icon.

Start a task with its play button or say “Start TASK-001.” Pause with the timer controls or “Pause TASK-001.” Several tasks can run timers at the same time; each running task gets its own color, and a **Running timers** strip shows them all with live clocks, pause and complete buttons. Prefer one at a time? Turn off **Settings → Timers → Let several tasks run timers at the same time**, and starting a task pauses the others again.

Timers stop automatically after a configurable cutoff (default **4 hours**; 30 minutes to 12 hours, or off) in **Settings → Timers**. A session that runs past the cutoff is recorded only up to the cutoff and marked **Auto-stopped** in Effort & time so it can be corrected. The cutoff is enforced when the app loads tasks and every 30 seconds by the menu bar thread, so a timer forgotten overnight never records a whole night. The strip warns when a running timer is within 15 minutes of its cutoff.

**Steps.** Every task has a checklist of small steps. Choose **✨ Break it down** in the task card and the writing model suggests 3–8 concrete steps, each 5–25 minutes, starting with a deliberately tiny first step; you can also add, check and remove steps yourself or ask in chat (“break TASK-004 into steps”). Checking off a step pops confetti from the checkbox with a soft chime, and a progress ring, gradient bar and step count (also shown on each task row) fill up. Finishing every step glows the card green and offers to complete the task; completing any task fires full-screen confetti. Confetti is skipped when macOS Reduce Motion is on, and chimes can be turned off in **Settings → Timers**. Completing, reopening, or trashing a running task saves elapsed time. In **Effort & time**, add focused minutes for today or a past date, edit a recorded session's duration, or remove a session. Manual logs count toward the chosen day's capacity and Focus Journal. These changes also refresh time calibration for completed tasks. The summary shows today/all-time totals. Sort by Time tracked, or ask chat about time spent. Timers continue while windows are closed and across app restarts. AppKit’s will-sleep notification pauses running timers before Mac sleep; waking does not automatically resume a task. Existing tasks load with zero tracked time and no running timer.

Sync between devices:

Open **Settings → Sync**, pick iCloud Drive, OneDrive, Dropbox or Google Drive from the folders found on this computer (or paste any synced folder path), name the device, and turn sync on. Do the same with the same folder on your other computer. Cadence creates a `Cadence Sync/devices/` folder; each device writes only its own `<device-id>.json` there, every few seconds when something changed, and merges the other devices' files. Because no two devices write the same file, the provider never has to resolve a write conflict, and any conflict copies it does create merge harmlessly. The SQLite database never goes into the cloud folder.

- **What syncs:** tasks, steps, timers (start on one device, stop on another), completion, labels, links and Trash. Chat history, capacity/calibration data, timer settings and API keys stay on each device.
- **Conflicts:** per task, the most recently edited version wins.
- **Identity:** every task has a hidden `uid`. Canvas assignments use their Canvas key, so two devices that both sync Canvas share one task per assignment. Task numbers are kept when free; if two devices create tasks before syncing, one of them gets a new number on the other device. Links between tasks follow uids, so epics and related items stay intact.
- **iCloud:** files iCloud has offloaded are requested with `brctl download` and merged on a later pass.
- Merged data is validated; invalid tasks are skipped and links that don't fit are dropped rather than blocking local edits.

The sidebar shows which folder you sync through, how many other devices it sees, and when it last synced. A phone app is not built yet: iOS access to iCloud Drive needs a mobile build with an iCloud container entitlement. A direct device-to-device link (QR pairing and public keys over MQTT) is a possible later transport using the same merge rules.

**Breaks.** While at least one timer runs, Cadence reminds you to drink water (every 45 minutes by default), rest your eyes with the 20-20-20 rule (every 20 minutes) and take a small brain break (every hour). Each reminder is a small banner at the top of the screen that floats above other apps, including full-screen ones on macOS; it drops in with a pulsing glow and a soft bell and closes on its own after 25 seconds (hovering pauses it). **Done** resets that reminder, **Snooze** brings it back in 10 minutes, **Skip** waits a full interval, and **Quiet 1h** pauses all of them. The eye reminder can guide a 20-second countdown. Only time with a running timer counts, a pause of 5 minutes or more counts as rest, and reminders are always at least 2 minutes apart. Change intervals, turn any reminder off, preview them, or pause for 1–3 hours in **Settings → Breaks**. The banner is a frameless, transparent, always-on-top Tauri window on macOS, Windows and Linux; on Linux, whether it stays above full-screen apps depends on the window manager.

Settings are grouped in a sidebar: **Focus** (Timers, Breaks), **Connections** (Assistant, Canvas, Sync) and **App** (Updates). Open them from **⚙ Settings** in the sidebar.

Updates: Cadence checks GitHub Releases every six hours and installs signed updates quietly, restarting into the new version (tasks are saved in SQLite, so nothing is lost). To check right away, choose **Cadence → Check for Updates…** on macOS or **Settings → Updates** on any platform. It shows your version and either confirms you're up to date or offers **Install & restart** with download progress and release notes. Development builds can check but never replace themselves.

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
