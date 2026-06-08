const IDLE_TIMEOUT = 5 * 60 * 1000;
let lastActivity = Date.now();
let idle = $state(false);
let timer: ReturnType<typeof setTimeout> | null = null;

function checkIdle() {
  idle = Date.now() - lastActivity > IDLE_TIMEOUT;
  if (!idle) schedule();
}

function schedule() {
  if (timer) clearTimeout(timer);
  timer = setTimeout(checkIdle, IDLE_TIMEOUT);
}

export function initIdle() {
  const events = ["mousemove", "mousedown", "keydown", "scroll", "touchstart", "wheel"];
  function poke() {
    lastActivity = Date.now();
    if (idle) idle = false;
    schedule();
  }
  for (const e of events) window.addEventListener(e, poke, { passive: true });
  schedule();
}

export function getIdleState() {
  return {
    get isIdle() { return idle; },
    setActive() { lastActivity = Date.now(); idle = false; schedule(); },
  };
}
