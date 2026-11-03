type TauriGlobal = { core: { invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> }; event: { listen<T>(event: string, handler: (event: { payload: T }) => void): Promise<() => void> } };
declare global { interface Window { __TAURI__?: TauriGlobal } }
export const desktop = () => typeof window !== 'undefined' && Boolean(window.__TAURI__);
export async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!window.__TAURI__) throw new Error('Open the desktop app to manage tasks. This browser preview only shows sample data.');
  return window.__TAURI__.core.invoke<T>(command, args);
}
export async function progress(handler: (text: string) => void): Promise<() => void> {
  if (!window.__TAURI__) return () => {};
  return window.__TAURI__.event.listen<string>('chat-progress', event => handler(event.payload));
}
export async function listen<T>(event: string, handler: (payload: T) => void): Promise<() => void> {
  if (!window.__TAURI__) return () => {};
  return window.__TAURI__.event.listen<T>(event, e => handler(e.payload));
}
