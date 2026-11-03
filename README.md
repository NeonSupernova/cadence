# Cadence

The desktop app is in [`desktop/`](desktop/README.md): Svelte UI, Rust task logic, SQLite persistence, and the same Jev + OpenRouter pipeline.

On macOS, double-click [`desktop/launch.command`](desktop/launch.command), or run:

```sh
cd desktop
bun install
bun run launch
```

It opens into Today / All tasks / Capacity / Focus Journal, with native Mac menus, a menu bar timer, Quick Capture, chat-driven task management, stable `TASK-001` references, task details, custom labels, deadlines, personal fit sorting, persistent task timers, system theme and native macOS materials, progress statistics, restorable Trash, and an isolated demo workspace. Your root `.env` is read at runtime, or you can configure OpenRouter in Settings. Canvas assignments can be synced with a personal token in Settings → Canvas; they are automatically labeled `canvas`. Personal workload estimates and a seven-day plan start with 2 hours on weekdays and 1 hour on weekends, then adapt using tracked work and explicit feedback; see [the workload model](desktop/WORKLOAD.md).

The frontend production build, Svelte checks, forty-four Rust tests, and strict Clippy pass. Native Rust compilation now passes with cached dependencies. GUI launch/visual checks require a normal desktop session. See the [desktop README](desktop/README.md) for details.

## Console prototype

Fast natural language task tracker with a colored terminal UI. Node 24+, no runtime dependencies.

1. Add your OpenRouter key to `.env`: `OPENROUTER_API_KEY=sk-or-...`
2. Run `npm start`.
3. Type a task: `Fix the checkout crash before Friday, it's blocking launch`.

Run `npm run demo` for a populated sample board without an API key. The demo displays sample tasks without saving them. The dashboard shows open/high priority/completed counts and a color coded table of IDs, status, priority, type, task title, and labels. It adapts to terminal width; `/show ID` displays full details. Adding or completing a task refreshes the board.

Jev classifies type, assigns multiple labels with Noul probabilities, and scores priority in one batched request. A second reasoning model writes the title and description using that classification. The task is saved only after both stages succeed. Open tasks sort by Jev priority (0–100), with oldest first on ties. Completed tasks follow open tasks. Data persists in `data/tasks.sqlite`.

```text
/list                  Open tasks, sorted by priority
/list all              Include completed tasks
/list #coding          Filter by label
/list type:bug         Filter by task type
/list checkout         Search task text
/show 1                Title, description, original input, classification
/done 1                Complete task
/open 1                Reopen task
/types                 List task types and labels
/quit                  Exit
```

Commands also work directly: `npm start -- /list all` or `npm start -- "Book a dentist appointment next week"`.

Customize `TASK_TYPES` and `LABELS` in `src/types.ts`. These definitions are sent directly to Jev. The same file defines typed Choice, Noul, Score, request, response, and task interfaces; API responses are validated at runtime. `LABEL_THRESHOLD` controls label inclusion (default 0.65). `OPENROUTER_MODEL` selects the writing model (default `google/gemini-2.5-flash` with low reasoning effort). `NO_COLOR=1` disables color. `.env` and local databases are ignored by Git.

For development: `npm install`, `npm run typecheck`, `npm test`. Tests mock API calls; live calls require your key and OpenRouter credits.

API references: [Jev on OpenRouter](https://openrouter.ai/docs/guides/community/jev), [Decisions tutorial and schemas](https://openrouter.ai/docs/guides/community/jev-tutorial), [OpenRouter reasoning](https://openrouter.ai/docs/guides/best-practices/reasoning-tokens).
