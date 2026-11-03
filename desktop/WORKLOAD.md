# Personal workload and capacity

The editable starting schedule is **120 focused minutes Monday–Friday and 60 Saturday–Sunday**. Today’s check-in overrides that day’s total available minutes and energy, rather than adding more time to an already used budget.

## What a task costs

Every task has an estimated total duration plus four 1–5 ratings: technical difficulty, tedium, ambiguity, and context switching. New natural-language tasks receive typed Jev assessments. You can edit these in task details or chat. Older tasks and imported Canvas assignments start with explicitly provisional defaults; “Assess with Jev” can refine an individual task. Canvas titles alone cannot establish assignment scope, so those estimates need review. User assessments are preserved.

A work unit (WU) starts as **25 focused minutes at moderate load**. Remaining WU = estimated remaining minutes / 25 × personal load factor. The load factor initially weights technical difficulty, tedium, ambiguity, and switching separately. Deadlines and priority affect ordering and deadline warnings, never make a hard task cost less.

Tracked time reduces estimated remaining time. When a task exceeds its estimate, the app retains a small remaining allowance and flags the estimate for review instead of pretending the unfinished task costs zero. Completed tasks show estimated total cost and retain their actual tracked time.

## How it learns

SQLite stores completion outcomes, effort ratings, daily check-ins, and explicit day reviews. Starting a timer freezes the baseline assessment so later estimate edits cannot silently rewrite that original benchmark.

Duration corrections use bounded, regularized Huber updates on log(actual / estimated duration). Features include task type, scope duration, and the four difficulty factors. The last 200 outcomes are replayed deterministically, with a 90-day half-life for duration and effort observations. Reading the app does not count an observation twice. A reopened task loses its completion outcome; completing it again replaces that task’s observation.

Subjective completion ratings—lighter than expected through draining—train separate, bounded factor weights. They can learn, for example, that tedious work costs more for you than technical work. Duration and subjective load are distinct signals.

Automatic duration observations require at least five minutes, at most sixteen hours total, and no individual session over four hours. Suspicious timers do not train duration. You can provide corrected actual minutes with completion feedback. Canvas submissions/grades alone never become timed learning samples.

Weekday capacity factors learn cautiously from explicit light/sustainable/overloaded day reviews with at least fifteen minutes tracked. A light review only increases capacity when most of the stated time was used. An untracked day is not interpreted as low ability. Weekday factors stay between 0.55 and 1.45; one review has a small bounded effect.

Pausing learning stops collection of new outcomes and ratings. Existing observations still inform estimates and retain their age weighting.

## How the seven-day plan works

Each day has two limits: focused minutes and work units, adjusted by energy and learned weekday capacity. Sessions are split at local midnight when counting daily work. Trashed tasks still count as time already spent.

A greedy scheduler orders ready tasks using deadline pressure, priority, and load, then allocates blocks within both budgets. Larger tasks can span days. Low-energy days defer heavy work unless a deadline is imminent. Blocked, completed, and trashed tasks are excluded. Deadline warnings compare work allocated by the actual due date with remaining scope.

The Today screen shows remaining capacity, suggested blocks, and deadline risk. Capacity shows the seven-day allocation, editable schedule, learning counts, factor weights, and day reviews. Task details expose the assumptions and rough total-time range. Chat can change estimates, daily time/energy, weekly capacity, and completion feedback using stable TASK references.

This is a transparent prototype, not an optimal calendar scheduler. Time ranges are rough uncertainty indicators, not statistical guarantees. It starts with broad defaults and few observations; useful personalization requires honest estimates, timers, and feedback. It does not currently learn separate course/label effects or infer free time from your calendar.
