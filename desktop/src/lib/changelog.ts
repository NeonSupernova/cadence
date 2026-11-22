/** Release notes shown in the What's New window. Newest first; write for people, not developers. */
export type Release = { version: string; date: string; title: string; notes: { icon: string; text: string }[] };

export const changelog: Release[] = [
  {
    version: '0.6.1', date: '2026-11-22', title: 'See what’s new',
    notes: [
      { icon: '📣', text: 'This window! After each update, Cadence shows you what changed.' },
      { icon: '🗂', text: 'Missed something? Open it any time from Settings → Updates.' },
    ],
  },
  {
    version: '0.6.0', date: '2026-11-22', title: 'Reminders and an easier way to add tasks',
    notes: [
      { icon: '🔔', text: 'Get a heads-up before tasks are due. Choose how early, set quiet hours, or pause alerts.' },
      { icon: '⏰', text: 'Set a reminder on any task, once or on repeat.' },
      { icon: '✍️', text: 'Add tasks yourself with the New task button. No assistant needed.' },
      { icon: '📌', text: 'Cadence now sits in your menu bar or system tray, showing your timer and what’s due today.' },
      { icon: '🔴', text: 'A badge on the app icon shows how many tasks are due.' },
      { icon: '🎓', text: 'Submit Canvas coursework, read feedback, and check grades without leaving Cadence.' },
    ],
  },
  {
    version: '0.5.0', date: '2026-11-15', title: 'A fresh new look',
    notes: [
      { icon: '✨', text: 'A brand-new design with soft, glassy colors.' },
      { icon: '📊', text: 'Charts show where your focus time went, today and this week.' },
      { icon: '🧠', text: 'The assistant is smarter, and chat keeps working even when part of it is down.' },
      { icon: '🎉', text: 'A little celebration when you finish things.' },
    ],
  },
  {
    version: '0.4.3', date: '2026-11-15', title: 'Take a break',
    notes: [
      { icon: '💧', text: 'Gentle reminders to drink water, rest your eyes, and stretch.' },
      { icon: '⚙️', text: 'Settings are now organized into clear sections.' },
    ],
  },
  {
    version: '0.4.2', date: '2026-11-15', title: 'Check for updates',
    notes: [{ icon: '⬇️', text: 'Check for a new version any time and install it right away.' }],
  },
  {
    version: '0.4.1', date: '2026-11-15', title: 'Smoother syncing',
    notes: [{ icon: '🛠', text: 'Fixed task numbers getting mixed up when syncing between devices.' }],
  },
  {
    version: '0.4.0', date: '2026-11-15', title: 'Juggle more at once',
    notes: [
      { icon: '⏱', text: 'Run more than one timer at a time. Forgotten timers stop on their own.' },
      { icon: '🪜', text: 'Break big tasks into small, doable steps.' },
      { icon: '☁️', text: 'Keep your tasks on every device with iCloud, OneDrive, or any synced folder.' },
    ],
  },
  {
    version: '0.3.0', date: '2026-11-12', title: 'Fix your time',
    notes: [
      { icon: '🕒', text: 'Add work time you forgot to track, or correct a session.' },
      { icon: '🐧', text: 'Cadence starts more reliably on Linux.' },
    ],
  },
  {
    version: '0.2.0', date: '2026-11-03', title: 'Automatic updates',
    notes: [
      { icon: '🔄', text: 'Cadence now keeps itself up to date.' },
      { icon: '🪟', text: 'No more flashing windows on Windows.' },
    ],
  },
  {
    version: '0.1.0', date: '2026-11-02', title: 'Hello, Cadence',
    notes: [
      { icon: '👋', text: 'Tell Cadence what needs doing in your own words, and it organizes it for you.' },
      { icon: '📅', text: 'A daily plan that fits the time and energy you actually have.' },
      { icon: '⏱', text: 'Focus timers and a journal of the work you’ve done.' },
      { icon: '🎓', text: 'Bring in your Canvas assignments.' },
    ],
  },
];

/** Compares dotted versions numerically: negative when a < b. */
export function compareVersions(a: string, b: string): number {
  const pa = a.split('.').map(n => parseInt(n, 10) || 0), pb = b.split('.').map(n => parseInt(n, 10) || 0);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) { const d = (pa[i] ?? 0) - (pb[i] ?? 0); if (d) return d; }
  return 0;
}
